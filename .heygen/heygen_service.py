import asyncio
import json
import time
import uuid
from datetime import datetime, timezone

import httpx
import redis
from fastapi import HTTPException
from sqlmodel import Session

from app.api.deps import CurrentUser, redis_connection_pool
from app.api.utils.gcp_bucket_util import upload_to_gcp
from app.core.config import settings
from app.core.logs import log
from app.crud import GeneratedAssetCRUD
from app.models import (
    GeneratedAsset,
    GeneratedAssetCreate,
    GeneratedAssetStatus,
    HeygenGenerationConfig,
)

logger = log


def get_text_to_video_status_key(user: CurrentUser, status_id: str) -> str:
    return f"{user.id}:{status_id}:text_to_video_generation_status"


def get_text_to_video_error_message_key(user: CurrentUser, status_id: str) -> str:
    return f"{user.id}:{status_id}:text_to_video_generation_error_message"


def get_generated_video_path(user_id: str, file_name: str) -> str:
    return f"{user_id}/generated_assets/{file_name}"


class HeygenTextToVideo:
    def __init__(
        self,
        db_session: Session,
        status_id: str,
        user: CurrentUser,
    ):
        self.db = db_session
        self.status_id = status_id
        self.user = user
        self.redis = redis.Redis(connection_pool=redis_connection_pool)
        if not settings.HEYGEN_API_KEY or not settings.HEYGEN_IMPORTED_ELEVENLABS_KEY_ID:
            raise HTTPException(status_code=500, detail="HEYGEN secrets are not configured")
        self.heygen_api = HeygenApi(api_key=settings.HEYGEN_API_KEY)
        self.crud = GeneratedAssetCRUD()

    def set_status(self, message: str, status: GeneratedAssetStatus, asset: GeneratedAsset) -> None:
        asset.status = status
        self.redis.set(
            name=get_text_to_video_status_key(user=self.user, status_id=self.status_id),
            value=json.dumps({"message": message, "status": status.value}),
            ex=10 * 60,
        )
        self.db.commit()
        self.db.refresh(asset)

    def set_error_message(self, error_message: str, asset: GeneratedAsset) -> None:
        asset.error_message = f"Error: {error_message}"
        self.redis.set(
            name=get_text_to_video_error_message_key(
                user=self.user,
                status_id=self.status_id,
            ),
            value=json.dumps({"error_message": error_message}),
            ex=10 * 60,
        )
        self.db.commit()
        self.db.refresh(asset)

    def create_video_record(self, confidant_id: str, title: str | None) -> GeneratedAsset:
        time_now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
        asset_in = GeneratedAssetCreate(
            confidant_id=confidant_id,
            owner_id=self.user.id,
            title=title or f"Video by {self.user.full_name} {time_now}",
        )
        asset = self.crud.create(
            db=self.db,
            asset_in=asset_in,
        )
        self.set_status("Starting text-to-video generation", GeneratedAssetStatus.STARTED, asset)
        return asset

    async def process_text_to_video(
        self,
        asset: GeneratedAsset,
        image_file: bytes,
        image_content_type: str,
        voice_id: str,
        script: str,
        video_orientation: str,
    ) -> None:
        try:
            self.set_status("Enabling voice", GeneratedAssetStatus.IN_PROGRESS_UPLOADING, asset)
            heygen_voice_id, voice_name = await self._enable_elevenlabs_voice(voice_id=voice_id)

            self.set_status("Uploading image", GeneratedAssetStatus.IN_PROGRESS_UPLOADING, asset)
            image_key = await self._upload_image_to_heygen(image_file=image_file, content_type=image_content_type)

            asset.generation_config = HeygenGenerationConfig(
                script=script,
                voice_id=voice_id,
                image_key=image_key,
                voice_name=voice_name,
                voice_heygen_id=heygen_voice_id,
                video_orientation=video_orientation,
            ).model_dump()

            self.set_status("Generating video", GeneratedAssetStatus.IN_PROGRESS_GENERATING, asset)
            video_id = await self._generate_video(image_key, heygen_voice_id, script, video_orientation, asset.id)
            asset.external_id = video_id

            self.set_status(
                "Video generation in progress. Polling for status.",
                GeneratedAssetStatus.POLLING,
                asset,
            )
            generated_video = await self._poll_for_video_completion(video_id=video_id)

            self.set_status(
                "Video generation complete. Processing video.",
                GeneratedAssetStatus.IN_PROGRESS_PROCESSING,
                asset,
            )
            video_path, thumbnail_path = await self._process_generated_video(
                asset_id=asset.id,
                video_data=generated_video,
            )
            asset.asset_path = video_path
            asset.thumbnail_path = thumbnail_path

            self.set_status("Video processed and saved.", GeneratedAssetStatus.COMPLETED, asset)
            self.db.commit()
            self.db.refresh(asset)

        except Exception as e:
            logger.error(f"Error during text-to-video process: {e}")
            self.set_error_message(str(e), asset)
            self.set_status("Error", GeneratedAssetStatus.ERROR, asset)
            raise e

    async def _enable_elevenlabs_voice(self, voice_id: str) -> tuple[str, str]:
        logger.info(f"Enabling ElevenLabs voice {voice_id} in Heygen.")
        key_id = settings.HEYGEN_IMPORTED_ELEVENLABS_KEY_ID
        voices_data = await self.heygen_api.list_voices(key_id=key_id)

        found_voice = None
        for voice in voices_data.get("data", {}).get("list", []):
            if voice.get("id") == voice_id:
                found_voice = voice
                break

        if not found_voice:
            raise HTTPException(status_code=404, detail=f"Voice {voice_id} not found in Heygen.")

        elevenlabs_voice_id: str = found_voice["id"]
        heygen_voice_id: str | None = found_voice["voice_id"]
        voice_name: str = found_voice["name"]

        if not found_voice.get("enabled"):
            logger.info(f"Voice {elevenlabs_voice_id} is not enabled. Enabling it now.")
            created_voice_id = await self.heygen_api.enable_voice(
                key_id=key_id,
                elevenlabs_voice_id=elevenlabs_voice_id,
                heygen_voice_id=heygen_voice_id,
                name=voice_name,
                enabled=True,
            )
            heygen_voice_id = created_voice_id
            logger.info(f"Voice {elevenlabs_voice_id} enabled successfully.")
        else:
            logger.info(f"Voice {elevenlabs_voice_id} is already enabled.")

        if not heygen_voice_id:
            raise HTTPException(status_code=404, detail=f"Missing Heygen voice_id for {voice_id}.")

        return heygen_voice_id, voice_name

    async def _upload_image_to_heygen(self, image_file: bytes, content_type: str) -> str:
        logger.info("Uploading image to Heygen.")

        upload_data = await self.heygen_api.upload_asset(
            data=image_file,
            content_type=content_type,
        )
        image_key = upload_data.get("data", {}).get("image_key")
        if not image_key:
            raise HTTPException(status_code=500, detail="Failed to upload image to Heygen.")

        logger.info(f"Image uploaded successfully. Image key: {image_key}")
        return image_key

    async def _generate_video(
        self,
        image_key: str,
        heygen_voice_id: str,
        script: str,
        video_orientation: str,
        asset_id: uuid.UUID,
    ) -> str:
        logger.info(f"Generating video in Heygen for image {image_key} and voice id {heygen_voice_id}")
        video_data = await self.heygen_api.create_video(
            image_key=image_key,
            script=script,
            voice_id=heygen_voice_id,
            video_title=f"text_to_video_{asset_id}",
            video_orientation=video_orientation,
        )
        video_id = video_data.get("data", {}).get("video_id")
        if not video_id:
            raise HTTPException(status_code=500, detail="Failed to generate video in Heygen.")

        logger.info(f"Video generation started. Video ID: {video_id}")
        return video_id

    async def _poll_for_video_completion(self, video_id: str) -> dict[str, str]:
        logger.info(f"Trigger polling for video_id: {video_id}")
        timeout = 600  # 10 minutes
        start_time = time.time()

        while time.time() - start_time < timeout:
            status_data = await self._try_get_video(video_id=video_id)
            status = status_data["status"]

            if status == GeneratedAssetStatus.COMPLETED:
                return status_data

            elif status == GeneratedAssetStatus.ERROR:
                error_message = status_data["error_message"]
                raise ValueError(error_message)

            await asyncio.sleep(3)

        raise ValueError(f"Video generation timed out after {timeout / 60} minutes.")

    async def _try_get_video(self, video_id: str) -> dict[str, str | None]:
        if not video_id:
            raise ValueError("Asset does not have a video_id (external_id).")

        video_status_data = await self.heygen_api.get_video(video_id)
        video_status = video_status_data.get("data", {}).get("status")

        if video_status == "completed":
            video_url = video_status_data.get("data", {}).get("video_url")
            thumbnail_url = video_status_data.get("data", {}).get("thumbnail_url")

            if not video_url or not thumbnail_url:
                error_message = "Video or thumbnail URL not found in Heygen response."
                logger.error(f"Video generation failed for {video_id}. Error: {error_message}")
                return {"status": GeneratedAssetStatus.ERROR, "error_message": error_message}

            return {
                "status": GeneratedAssetStatus.COMPLETED,
                "video_url": video_url,
                "thumbnail_url": thumbnail_url,
            }

        elif video_status in ["pending", "processing"]:
            logger.info(f"Video {video_id} generation is still in progress with status: {video_status}.")
            return {"status": GeneratedAssetStatus.POLLING}

        else:
            error_details = video_status_data.get("data", {}).get("error")
            error_message = f"Video generation failed: {error_details or 'Unknown error'}"
            logger.error(f"Video generation failed for {video_id}. Status: {video_status}. Error: {error_details}")
            return {"status": GeneratedAssetStatus.ERROR, "error_message": error_message}

    async def _process_generated_video(self, asset_id: uuid.UUID, video_data: dict[str, str]) -> tuple[str, str]:
        video_url = video_data["video_url"]
        thumbnail_url = video_data["thumbnail_url"]

        gcs_video_path = get_generated_video_path(user_id=self.user.id, file_name=f"generated_video_{asset_id}.mp4")
        gcs_thumbnail_path = get_generated_video_path(
            user_id=self.user.id,
            file_name=f"generated_video_thumbnail_{asset_id}.jpeg",
        )

        await self._download_and_store_asset(video_url, gcs_video_path)
        await self._download_and_store_asset(thumbnail_url, gcs_thumbnail_path)

        return gcs_video_path, gcs_thumbnail_path

    async def _download_and_store_asset(self, url: str, gcp_path: str):
        async with httpx.AsyncClient() as client:
            file_response = await client.get(url, timeout=httpx.Timeout(10.0, read=120.0))
            file_response.raise_for_status()
            content = file_response.content

        result = await upload_to_gcp(
            file=content,
            bucket_name=settings.USERS_BUCKET,
            destination_blob_name=gcp_path,
            original_filename=url,
            user_info=self.user.full_name or self.user.email or "",
        )
        if not result.success:
            raise ValueError(result.error_message)


class HeygenApi:
    def __init__(self, api_key: str):
        self.api_v1_url = "https://api.heygen.com/v1"
        self.api_v2_url = "https://api.heygen.com/v2"
        self.api2_url = "https://api2.heygen.com/v1"
        self.upload_url = "https://upload.heygen.com/v1"
        self.headers = {
            "accept": "application/json",
            "x-api-key": api_key,
        }

    async def list_voices(self, key_id: str) -> dict:
        """Lists available voices from Heygen (via ElevenLabs)."""
        url = f"{self.api2_url}/third_party/eleven_labs/voice.list"
        params = {"key_id": key_id}
        async with httpx.AsyncClient() as client:
            response = await client.get(url, headers=self.headers, params=params, timeout=60)
            response.raise_for_status()
            return response.json()

    async def enable_voice(
        self,
        key_id: str,
        elevenlabs_voice_id: str,
        heygen_voice_id: str | None,
        name: str,
        enabled: bool = True,
    ) -> str:
        """Enables or disables a voice in Heygen."""
        url = f"{self.api2_url}/third_party/voice.enable"
        headers = self.headers.copy()
        headers["content-type"] = "application/json"
        data = {
            "key_id": key_id,
            "id": elevenlabs_voice_id,
            "voice_id": heygen_voice_id,
            "name": name,
            "enabled": enabled,
        }
        async with httpx.AsyncClient() as client:
            response = await client.post(url, headers=headers, json=data, timeout=60)
            response.raise_for_status()
            response_data = response.json()
            return response_data["data"]["voice_id"]

    async def upload_asset(self, data: bytes, content_type: str) -> dict:
        """Uploads an asset (e.g., image) to Heygen."""
        url = f"{self.upload_url}/asset"
        headers = self.headers.copy()
        headers["Content-Type"] = content_type
        async with httpx.AsyncClient() as client:
            response = await client.post(url, headers=headers, data=data, timeout=60)
            response.raise_for_status()
            return response.json()

    async def create_video(
        self,
        image_key: str,
        script: str,
        voice_id: str,
        video_title: str,
        video_orientation: str,
        fit: str = "contain",
    ) -> dict:
        """Creates a video from an image, script, and voice."""
        url = f"{self.api_v2_url}/video/av4/generate"
        headers = self.headers.copy()
        headers["content-type"] = "application/json"
        data = {
            "video_orientation": video_orientation,
            "image_key": image_key,
            "video_title": video_title,
            "script": script,
            "voice_id": voice_id,
            "fit": fit,
        }
        async with httpx.AsyncClient() as client:
            response = await client.post(url, headers=headers, json=data, timeout=60)
            response.raise_for_status()
            return response.json()

    async def get_video(self, video_id: str) -> dict:
        """Gets the status of a video generation task."""
        url = f"{self.api_v1_url}/video_status.get"
        params = {"video_id": video_id}
        async with httpx.AsyncClient() as client:
            response = await client.get(url, headers=self.headers, params=params, timeout=60)
            response.raise_for_status()
            return response.json()

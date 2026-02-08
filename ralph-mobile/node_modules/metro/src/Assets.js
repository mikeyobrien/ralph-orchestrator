"use strict";

const AssetPaths = require("./node-haste/lib/AssetPaths");
const crypto = require("crypto");
const fs = require("fs");
const getImageSize = require("image-size");
const path = require("path");
function isAssetTypeAnImage(type) {
  return (
    [
      "png",
      "jpg",
      "jpeg",
      "bmp",
      "gif",
      "webp",
      "psd",
      "svg",
      "tiff",
      "ktx",
    ].indexOf(type) !== -1
  );
}
function getAssetSize(type, content, filePath) {
  if (!isAssetTypeAnImage(type)) {
    return null;
  }
  if (content.length === 0) {
    throw new Error(`Image asset \`${filePath}\` cannot be an empty file.`);
  }
  const { width, height } = getImageSize(content);
  return {
    width,
    height,
  };
}
function buildAssetMap(dir, files, platform) {
  const platforms = new Set(platform != null ? [platform] : []);
  const assets = files.map((file) => AssetPaths.tryParse(file, platforms));
  const map = new Map();
  assets.forEach(function (asset, i) {
    if (asset == null) {
      return;
    }
    const file = files[i];
    const assetKey = getAssetKey(asset.assetName, asset.platform);
    let record = map.get(assetKey);
    if (!record) {
      record = {
        scales: [],
        files: [],
      };
      map.set(assetKey, record);
    }
    let insertIndex;
    const length = record.scales.length;
    for (insertIndex = 0; insertIndex < length; insertIndex++) {
      if (asset.resolution < record.scales[insertIndex]) {
        break;
      }
    }
    record.scales.splice(insertIndex, 0, asset.resolution);
    record.files.splice(insertIndex, 0, path.join(dir, file));
  });
  return map;
}
function getAssetKey(assetName, platform) {
  if (platform != null) {
    return `${assetName} : ${platform}`;
  } else {
    return assetName;
  }
}
async function getAbsoluteAssetRecord(assetPath, platform = null) {
  const filename = path.basename(assetPath);
  const dir = path.dirname(assetPath);
  const files = await fs.promises.readdir(dir);
  const assetData = AssetPaths.parse(
    filename,
    new Set(platform != null ? [platform] : [])
  );
  const map = buildAssetMap(dir, files, platform);
  let record;
  if (platform != null) {
    record =
      map.get(getAssetKey(assetData.assetName, platform)) ||
      map.get(assetData.assetName);
  } else {
    record = map.get(assetData.assetName);
  }
  if (!record) {
    throw new Error(
      `Asset not found: ${assetPath} for platform: ${
        platform ?? "(unspecified)"
      }`
    );
  }
  return record;
}
async function getAbsoluteAssetInfo(assetPath, platform = null) {
  const nameData = AssetPaths.parse(
    assetPath,
    new Set(platform != null ? [platform] : [])
  );
  const { name, type } = nameData;
  const { scales, files } = await getAbsoluteAssetRecord(assetPath, platform);
  const hasher = crypto.createHash("md5");
  const fileData = await Promise.all(
    files.map((file) => fs.promises.readFile(file))
  );
  for (const data of fileData) {
    hasher.update(data);
  }
  return {
    files,
    hash: hasher.digest("hex"),
    name,
    scales,
    type,
  };
}
async function getAssetData(
  assetPath,
  localPath,
  assetDataPlugins,
  platform = null,
  publicPath
) {
  let assetUrlPath = localPath.startsWith("..")
    ? publicPath.replace(/\/$/, "") + "/" + path.dirname(localPath)
    : path.join(publicPath, path.dirname(localPath));
  if (path.sep === "\\") {
    assetUrlPath = assetUrlPath.replaceAll("\\", "/");
  }
  const isImage = isAssetTypeAnImage(path.extname(assetPath).slice(1));
  const assetInfo = await getAbsoluteAssetInfo(assetPath, platform);
  const isImageInput = assetInfo.files[0].includes(".zip/")
    ? fs.readFileSync(assetInfo.files[0])
    : assetInfo.files[0];
  const dimensions = isImage ? getImageSize(isImageInput) : null;
  const scale = assetInfo.scales[0];
  const assetData = {
    __packager_asset: true,
    fileSystemLocation: path.dirname(assetPath),
    httpServerLocation: assetUrlPath,
    width: dimensions ? dimensions.width / scale : undefined,
    height: dimensions ? dimensions.height / scale : undefined,
    scales: assetInfo.scales,
    files: assetInfo.files,
    hash: assetInfo.hash,
    name: assetInfo.name,
    type: assetInfo.type,
  };
  return await applyAssetDataPlugins(assetDataPlugins, assetData);
}
async function applyAssetDataPlugins(assetDataPlugins, assetData) {
  if (!assetDataPlugins.length) {
    return assetData;
  }
  const [currentAssetPlugin, ...remainingAssetPlugins] = assetDataPlugins;
  const assetPluginFunction = require(currentAssetPlugin);
  const resultAssetData = await assetPluginFunction(assetData);
  return await applyAssetDataPlugins(remainingAssetPlugins, resultAssetData);
}
async function getAssetFiles(assetPath, platform = null) {
  const assetData = await getAbsoluteAssetRecord(assetPath, platform);
  return assetData.files;
}
async function getAsset(
  relativePath,
  projectRoot,
  watchFolders,
  platform = null,
  assetExts
) {
  const assetData = AssetPaths.parse(
    relativePath,
    new Set(platform != null ? [platform] : [])
  );
  const absolutePath = path.resolve(projectRoot, relativePath);
  if (!assetExts.includes(assetData.type)) {
    throw new Error(
      `'${relativePath}' cannot be loaded as its extension is not registered in assetExts`
    );
  }
  if (!pathBelongsToRoots(absolutePath, [projectRoot, ...watchFolders])) {
    throw new Error(
      `'${relativePath}' could not be found, because it cannot be found in the project root or any watch folder`
    );
  }
  const record = await getAbsoluteAssetRecord(absolutePath, platform);
  for (let i = 0; i < record.scales.length; i++) {
    if (record.scales[i] >= assetData.resolution) {
      return fs.promises.readFile(record.files[i]);
    }
  }
  return fs.promises.readFile(record.files[record.files.length - 1]);
}
function pathBelongsToRoots(pathToCheck, roots) {
  for (const rootFolder of roots) {
    if (pathToCheck.startsWith(path.resolve(rootFolder))) {
      return true;
    }
  }
  return false;
}
module.exports = {
  getAsset,
  getAssetSize,
  getAssetData,
  getAssetFiles,
  isAssetTypeAnImage,
};

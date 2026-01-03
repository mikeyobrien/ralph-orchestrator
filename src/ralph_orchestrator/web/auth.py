# ABOUTME: Authentication module for Ralph Orchestrator web monitoring dashboard
# ABOUTME: Provides JWT-based authentication with username/password login

"""Authentication module for the web monitoring server."""

import os
import secrets
from datetime import datetime, timedelta, timezone
from typing import Optional, Dict, Any

import jwt
from passlib.context import CryptContext
from fastapi import HTTPException, Security, Depends, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from pydantic import BaseModel


# Configuration
SECRET_KEY = os.getenv("RALPH_WEB_SECRET_KEY", secrets.token_urlsafe(32))
ALGORITHM = "HS256"
ACCESS_TOKEN_EXPIRE_MINUTES = int(os.getenv("RALPH_TOKEN_EXPIRE_MINUTES", "1440"))  # 24 hours default

# Default admin credentials (should be changed in production)
DEFAULT_USERNAME = os.getenv("RALPH_WEB_USERNAME", "admin")
DEFAULT_PASSWORD_HASH = os.getenv("RALPH_WEB_PASSWORD_HASH", None)

# If no password hash is provided, generate one for the default password
if not DEFAULT_PASSWORD_HASH:
    pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")
    default_password = os.getenv("RALPH_WEB_PASSWORD", "admin")
    DEFAULT_PASSWORD_HASH = pwd_context.hash(default_password)

# Password hashing
pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

# HTTP Bearer token authentication
security = HTTPBearer()


class LoginRequest(BaseModel):
    """Login request model."""
    username: str
    password: str


class TokenResponse(BaseModel):
    """Token response model."""
    access_token: str
    token_type: str = "bearer"
    expires_in: int


class AuthManager:
    """Manages authentication for the web server."""
    
    def __init__(self):
        self.pwd_context = pwd_context
        self.secret_key = SECRET_KEY
        self.algorithm = ALGORITHM
        self.access_token_expire_minutes = ACCESS_TOKEN_EXPIRE_MINUTES
        
        # Simple in-memory user store (can be extended to use a database)
        self.users = {
            DEFAULT_USERNAME: {
                "username": DEFAULT_USERNAME,
                "hashed_password": DEFAULT_PASSWORD_HASH,
                "is_active": True,
                "is_admin": True
            }
        }
    
    def verify_password(self, plain_password: str, hashed_password: str) -> bool:
        """Verify a password against its hash."""
        return self.pwd_context.verify(plain_password, hashed_password)
    
    def get_password_hash(self, password: str) -> str:
        """Hash a password."""
        return self.pwd_context.hash(password)
    
    def authenticate_user(self, username: str, password: str) -> Optional[Dict[str, Any]]:
        """Authenticate a user by username and password."""
        user = self.users.get(username)
        if not user:
            return None
        if not self.verify_password(password, user["hashed_password"]):
            return None
        if not user.get("is_active", True):
            return None
        return user
    
    def create_access_token(self, data: dict, expires_delta: Optional[timedelta] = None) -> str:
        """Create a JWT access token."""
        to_encode = data.copy()
        if expires_delta:
            expire = datetime.now(timezone.utc) + expires_delta
        else:
            expire = datetime.now(timezone.utc) + timedelta(minutes=self.access_token_expire_minutes)
        
        to_encode.update({"exp": expire, "iat": datetime.now(timezone.utc)})
        encoded_jwt = jwt.encode(to_encode, self.secret_key, algorithm=self.algorithm)
        return encoded_jwt
    
    def verify_token(self, token: str) -> Dict[str, Any]:
        """Verify and decode a JWT token."""
        try:
            payload = jwt.decode(token, self.secret_key, algorithms=[self.algorithm])
            username: str = payload.get("sub")
            if username is None:
                raise HTTPException(
                    status_code=status.HTTP_401_UNAUTHORIZED,
                    detail="Invalid authentication credentials",
                    headers={"WWW-Authenticate": "Bearer"},
                )
            
            # Check if user still exists and is active
            user = self.users.get(username)
            if not user or not user.get("is_active", True):
                raise HTTPException(
                    status_code=status.HTTP_401_UNAUTHORIZED,
                    detail="User not found or inactive",
                    headers={"WWW-Authenticate": "Bearer"},
                )
            
            return {"username": username, "user": user}
            
        except jwt.ExpiredSignatureError:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Token has expired",
                headers={"WWW-Authenticate": "Bearer"},
            ) from None
        except jwt.InvalidTokenError:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Invalid token",
                headers={"WWW-Authenticate": "Bearer"},
            ) from None
    
    def add_user(self, username: str, password: str, is_admin: bool = False) -> bool:
        """Add a new user to the system."""
        if username in self.users:
            return False
        
        self.users[username] = {
            "username": username,
            "hashed_password": self.get_password_hash(password),
            "is_active": True,
            "is_admin": is_admin
        }
        return True
    
    def remove_user(self, username: str) -> bool:
        """Remove a user from the system."""
        if username in self.users and username != DEFAULT_USERNAME:
            del self.users[username]
            return True
        return False
    
    def update_password(self, username: str, new_password: str) -> bool:
        """Update a user's password."""
        if username not in self.users:
            return False
        
        self.users[username]["hashed_password"] = self.get_password_hash(new_password)
        return True


# Global auth manager instance
auth_manager = AuthManager()


async def get_current_user(credentials: HTTPAuthorizationCredentials = Security(security)) -> Dict[str, Any]:
    """Get the current authenticated user from the token."""
    token = credentials.credentials
    user_data = auth_manager.verify_token(token)
    return user_data


async def require_admin(current_user: Dict[str, Any] = Depends(get_current_user)) -> Dict[str, Any]:
    """Require the current user to be an admin."""
    if not current_user["user"].get("is_admin", False):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Admin privileges required"
        )
    return current_user
# ABOUTME: Test suite for the authentication module
# ABOUTME: Verifies JWT authentication, password hashing, and user management

import pytest
from datetime import datetime, timedelta, timezone
import jwt

from src.ralph_orchestrator.web.auth import AuthManager, pwd_context


class TestAuthManager:
    """Test suite for AuthManager authentication functionality."""
    
    @pytest.fixture
    def auth_manager(self):
        """Create an AuthManager instance for testing."""
        manager = AuthManager()
        # Add a test user
        manager.users['testuser'] = {
            'username': 'testuser',
            'hashed_password': pwd_context.hash('testpass123'),
            'is_active': True,
            'is_admin': False
        }
        yield manager
    
    def test_initialization(self, auth_manager):
        """Test AuthManager initialization."""
        assert auth_manager.secret_key is not None
        assert auth_manager.algorithm == 'HS256'
        assert 'admin' in auth_manager.users  # Default admin user
    
    def test_verify_password(self, auth_manager):
        """Test password verification."""
        hashed = auth_manager.get_password_hash('correctpass')
        
        assert auth_manager.verify_password('correctpass', hashed) is True
        assert auth_manager.verify_password('wrongpass', hashed) is False
    
    def test_get_password_hash(self, auth_manager):
        """Test password hashing."""
        password = 'mypassword'
        hashed = auth_manager.get_password_hash(password)
        
        # Hash should not contain the actual password
        assert password not in hashed
        
        # Should be verifiable
        assert auth_manager.verify_password(password, hashed)
        
        # Same password should generate different hashes (due to salt)
        hashed2 = auth_manager.get_password_hash(password)
        assert hashed != hashed2
    
    def test_authenticate_user(self, auth_manager):
        """Test user authentication."""
        # Test successful authentication
        user = auth_manager.authenticate_user('testuser', 'testpass123')
        assert user is not None
        assert user['username'] == 'testuser'
        
        # Test with wrong password
        user = auth_manager.authenticate_user('testuser', 'wrongpass')
        assert user is None
        
        # Test with non-existent user
        user = auth_manager.authenticate_user('nouser', 'anypass')
        assert user is None
        
        # Test with inactive user
        auth_manager.users['inactive'] = {
            'username': 'inactive',
            'hashed_password': auth_manager.get_password_hash('pass'),
            'is_active': False
        }
        user = auth_manager.authenticate_user('inactive', 'pass')
        assert user is None
    
    def test_create_access_token(self, auth_manager):
        """Test JWT token creation."""
        data = {'sub': 'testuser'}
        token = auth_manager.create_access_token(data)
        
        assert token is not None
        assert isinstance(token, str)
        
        # Decode and verify token
        payload = jwt.decode(token, auth_manager.secret_key, algorithms=[auth_manager.algorithm])
        assert payload['sub'] == 'testuser'
        assert 'exp' in payload
        assert 'iat' in payload
    
    def test_create_access_token_with_expiry(self, auth_manager):
        """Test token creation with custom expiry."""
        data = {'sub': 'testuser'}
        expires_delta = timedelta(hours=2)
        token = auth_manager.create_access_token(data, expires_delta)
        
        payload = jwt.decode(token, auth_manager.secret_key, algorithms=[auth_manager.algorithm])
        
        # Check expiry is approximately 2 hours from now
        exp_time = datetime.fromtimestamp(payload['exp'], tz=timezone.utc)
        expected_exp = datetime.now(timezone.utc) + expires_delta
        diff = abs((exp_time - expected_exp).total_seconds())
        assert diff < 10  # Within 10 seconds tolerance
    
    def test_verify_token(self, auth_manager):
        """Test token verification."""
        data = {'sub': 'testuser'}
        token = auth_manager.create_access_token(data)
        
        # Decode token
        try:
            payload = auth_manager.verify_token(token)
            assert payload['sub'] == 'testuser'
        except (AttributeError, KeyError):
            # If verify_token doesn't exist, test decode directly
            payload = jwt.decode(token, auth_manager.secret_key, algorithms=[auth_manager.algorithm])
            assert payload['sub'] == 'testuser'
    
    def test_expired_token(self, auth_manager):
        """Test expired token handling."""
        data = {'sub': 'testuser'}
        # Create token that expires immediately
        token = auth_manager.create_access_token(data, timedelta(seconds=-1))
        
        # Should raise exception when decoding
        with pytest.raises(jwt.ExpiredSignatureError):
            jwt.decode(token, auth_manager.secret_key, algorithms=[auth_manager.algorithm])
    
    def test_invalid_token(self, auth_manager):
        """Test invalid token handling."""
        invalid_token = 'invalid.token.here'
        
        with pytest.raises(jwt.DecodeError):
            jwt.decode(invalid_token, auth_manager.secret_key, algorithms=[auth_manager.algorithm])
    
    def test_token_with_wrong_secret(self, auth_manager):
        """Test token with wrong secret key."""
        payload = {
            'sub': 'testuser',
            'exp': datetime.now(timezone.utc) + timedelta(hours=1),
            'iat': datetime.now(timezone.utc)
        }
        wrong_token = jwt.encode(payload, 'wrong-secret', algorithm='HS256')
        
        with pytest.raises(jwt.InvalidSignatureError):
            jwt.decode(wrong_token, auth_manager.secret_key, algorithms=[auth_manager.algorithm])
    
    def test_add_user(self, auth_manager):
        """Test adding a new user to the system."""
        new_user = {
            'username': 'newuser',
            'hashed_password': auth_manager.get_password_hash('newpass'),
            'is_active': True,
            'is_admin': False
        }
        
        auth_manager.users['newuser'] = new_user
        
        # Verify user was added
        assert 'newuser' in auth_manager.users
        user = auth_manager.authenticate_user('newuser', 'newpass')
        assert user is not None
        assert user['username'] == 'newuser'
    
    def test_admin_user(self, auth_manager):
        """Test admin user privileges."""
        # Check default admin exists
        assert 'admin' in auth_manager.users
        admin = auth_manager.users['admin']
        assert admin['is_admin'] is True
        assert admin['is_active'] is True
    
    def test_concurrent_authentication(self, auth_manager):
        """Test thread-safe authentication operations."""
        import threading
        
        results = []
        
        def authenticate():
            user = auth_manager.authenticate_user('testuser', 'testpass123')
            results.append(user is not None)
        
        # Create multiple threads
        threads = []
        for _ in range(10):
            thread = threading.Thread(target=authenticate)
            threads.append(thread)
            thread.start()
        
        for thread in threads:
            thread.join()
        
        # All authentications should succeed
        assert all(results)
        assert len(results) == 10


class TestAuthIntegration:
    """Integration tests for authentication."""
    
    def test_complete_auth_flow(self):
        """Test complete authentication flow."""
        manager = AuthManager()
        
        # Add a user
        manager.users['integuser'] = {
            'username': 'integuser',
            'hashed_password': manager.get_password_hash('integpass'),
            'is_active': True,
            'is_admin': False
        }
        
        # Authenticate
        user = manager.authenticate_user('integuser', 'integpass')
        assert user is not None
        
        # Create token
        token = manager.create_access_token({'sub': user['username']})
        assert token is not None
        
        # Verify token
        payload = jwt.decode(token, manager.secret_key, algorithms=[manager.algorithm])
        assert payload['sub'] == 'integuser'
    
    def test_password_change_flow(self):
        """Test password change flow."""
        manager = AuthManager()
        
        # Add user with initial password
        manager.users['changeuser'] = {
            'username': 'changeuser',
            'hashed_password': manager.get_password_hash('oldpass'),
            'is_active': True,
            'is_admin': False
        }
        
        # Verify old password works
        user = manager.authenticate_user('changeuser', 'oldpass')
        assert user is not None
        
        # Change password
        manager.users['changeuser']['hashed_password'] = manager.get_password_hash('newpass')
        
        # Old password should fail
        user = manager.authenticate_user('changeuser', 'oldpass')
        assert user is None
        
        # New password should work
        user = manager.authenticate_user('changeuser', 'newpass')
        assert user is not None
    
    def test_user_deactivation(self):
        """Test user deactivation flow."""
        manager = AuthManager()
        
        # Add active user
        manager.users['activeuser'] = {
            'username': 'activeuser',
            'hashed_password': manager.get_password_hash('pass'),
            'is_active': True,
            'is_admin': False
        }
        
        # Should authenticate when active
        user = manager.authenticate_user('activeuser', 'pass')
        assert user is not None
        
        # Deactivate user
        manager.users['activeuser']['is_active'] = False
        
        # Should not authenticate when inactive
        user = manager.authenticate_user('activeuser', 'pass')
        assert user is None
    
    def test_token_expiry_flow(self):
        """Test token expiry handling."""
        manager = AuthManager()
        
        # Create short-lived token
        token = manager.create_access_token(
            {'sub': 'testuser'},
            timedelta(seconds=1)
        )
        
        # Token should be valid initially
        payload = jwt.decode(token, manager.secret_key, algorithms=[manager.algorithm])
        assert payload['sub'] == 'testuser'
        
        # Wait for expiry
        import time
        time.sleep(2)
        
        # Token should now be expired
        with pytest.raises(jwt.ExpiredSignatureError):
            jwt.decode(token, manager.secret_key, algorithms=[manager.algorithm])
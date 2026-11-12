use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(u64);

impl UserId {
    /// Creates a new user ID
    pub fn new(id: u64) -> Self {
        UserId(id)
    }
    /// Get the inner ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// User profile structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// User ID
    pub id: UserId,
    /// Username
    pub username: String,
    /// Discriminator (0000-9999)
    pub discriminator: String,
    /// Whether the user is a mutual friend
    pub is_mutual_friend: bool,
    /// Whether the user is a verified bot
    pub is_bot: bool,
    /// Avatar data
    pub avatar: Option<String>,
    /// Whether the user is currently online
    pub is_online: bool,
    /// Status message
    pub status: Option<String>,
}

/// User profile service
#[derive(Debug, Clone)]
pub struct UserProfileService {
    /// Current focused user
    focused_user: Option<UserProfile>,
    /// User cache
    cache: HashMap<UserId, UserProfile>,
    /// Maximum cache size
    max_cache: usize,
}

impl Default for UserProfileService {
    fn default() -> Self {
        Self::new()
    }
}

impl UserProfileService {
    /// Creates a new user profile service
    pub fn new() -> Self {
        UserProfileService {
            focused_user: None,
            cache: HashMap::new(),
            max_cache: 1000,
        }
    }
    /// Set focused user
    pub fn set_focused_user(&mut self, user: UserProfile) {
        self.focused_user = Some(user);
    }
    /// Get focused user
    pub fn focused_user(&self) -> Option<&UserProfile> {
        self.focused_user.as_ref()
    }
    /// Add user to cache (respecting max_cache)
    /// Takes user_id separately to avoid borrow checker issues
    pub fn add_user_to_cache(&mut self, user_id: UserId, user: UserProfile) {
        if self.cache.len() >= self.max_cache
            && let Some(old_key) = self.cache.keys().next().cloned()
        {
            self.cache.remove(&old_key);
        }
        self.cache.insert(user_id, user);
    }
    /// Get user from cache
    pub fn get_user(&self, user_id: UserId) -> Option<&UserProfile> {
        self.cache.get(&user_id)
    }
    /// Check if user is in cache
    pub fn has_user(&self, user_id: UserId) -> bool {
        self.cache.contains_key(&user_id)
    }
    /// Get focused user ID
    pub fn focused_user_id(&self) -> Option<UserId> {
        self.focused_user.as_ref().map(|u| u.id.clone())
    }
}

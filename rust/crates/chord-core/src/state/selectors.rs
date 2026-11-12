//! P2-02c: селекторы состояния (RW-13 / W13c).
//!
//! Все выборки — только чтение и без паники. Порядок детерминирован: гильдии/пользователи/
//! сообщения/треды — по возрастанию id (`BTreeMap`), каналы гильдии — `(position, id)`.

use crate::ready::{
    Channel, Guild, GuildMember, PrivateChannel, ReadStateEntry, Relationship, User,
    UserGuildSettingsEntry,
};

use super::types::{Message, State, Thread};

impl State {
    /// Гильдия по id.
    #[must_use]
    pub fn guild(&self, guild_id: u64) -> Option<&Guild> {
        self.guilds.get(&guild_id)
    }

    /// Канал гильдии по id.
    #[must_use]
    pub fn channel(&self, channel_id: u64) -> Option<&Channel> {
        let guild_id = *self.channel_guild.get(&channel_id)?;
        self.guilds
            .get(&guild_id)?
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
    }

    /// Гильдия канала (если канал известен).
    #[must_use]
    pub fn channel_guild_id(&self, channel_id: u64) -> Option<u64> {
        self.channel_guild.get(&channel_id).copied()
    }

    /// Каналы гильдии в UI-порядке `(position, id)`.
    #[must_use]
    pub fn channels_in_guild(&self, guild_id: u64) -> Vec<&Channel> {
        let mut channels: Vec<&Channel> = self
            .guilds
            .get(&guild_id)
            .map(|guild| guild.channels.iter().collect())
            .unwrap_or_default();
        channels.sort_unstable_by_key(|channel| (channel.position, channel.id));
        channels
    }

    /// Приватный канал (ЛС/группа) по id.
    #[must_use]
    pub fn private_channel(&self, channel_id: u64) -> Option<&PrivateChannel> {
        self.private_channels.get(&channel_id)
    }

    /// Пользователь по id.
    #[must_use]
    pub fn user(&self, user_id: u64) -> Option<&User> {
        self.users.get(&user_id)
    }

    /// Связь по id.
    #[must_use]
    pub fn relationship(&self, relationship_id: u64) -> Option<&Relationship> {
        self.relationships.get(&relationship_id)
    }

    /// Сообщение по id.
    #[must_use]
    pub fn message(&self, message_id: u64) -> Option<&Message> {
        self.messages.get(&message_id)
    }

    /// Сообщения канала по возрастанию id.
    #[must_use]
    pub fn messages_in_channel(&self, channel_id: u64) -> Vec<&Message> {
        self.messages
            .values()
            .filter(|message| message.channel_id == channel_id)
            .collect()
    }

    /// Тред по id.
    #[must_use]
    pub fn thread(&self, thread_id: u64) -> Option<&Thread> {
        self.threads.get(&thread_id)
    }

    /// Треды гильдии по возрастанию id.
    #[must_use]
    pub fn threads_in_guild(&self, guild_id: u64) -> Vec<&Thread> {
        self.threads
            .values()
            .filter(|thread| thread.guild_id == guild_id)
            .collect()
    }

    /// Участник гильдии по user_id.
    #[must_use]
    pub fn guild_member(&self, guild_id: u64, user_id: u64) -> Option<&GuildMember> {
        self.guilds
            .get(&guild_id)?
            .members
            .iter()
            .find(|member| member.user_id == user_id)
    }

    /// Запись read-state по id.
    #[must_use]
    pub fn read_state_entry(&self, id: u64) -> Option<&ReadStateEntry> {
        self.read_state.entries.iter().find(|entry| entry.id == id)
    }

    /// Настройки гильдии по guild_id.
    #[must_use]
    pub fn user_guild_settings_entry(&self, guild_id: u64) -> Option<&UserGuildSettingsEntry> {
        self.user_guild_settings
            .entries
            .iter()
            .find(|entry| entry.guild_id == Some(guild_id))
    }

    /// Число гильдий.
    #[must_use]
    pub fn guild_count(&self) -> usize {
        self.guilds.len()
    }

    /// Число каналов гильдий (без приватных и тредов).
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.guilds.values().map(|guild| guild.channels.len()).sum()
    }

    /// Число приватных каналов.
    #[must_use]
    pub fn private_channel_count(&self) -> usize {
        self.private_channels.len()
    }

    /// Число известных пользователей.
    #[must_use]
    pub fn user_count(&self) -> usize {
        self.users.len()
    }

    /// Число сообщений.
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Число тредов.
    #[must_use]
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
}

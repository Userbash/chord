Varint/bounds, wrapper presence, signed timezone, fixed64 parsing, unknown preservation, type 1/2 mapping и независимость тестов соответствуют контракту; заявленные green-прогоны также подтверждаются логами. Однако Debug секций может раскрыть id и локализационные значения, поэтому требуется исправление P2.

Review comment:

- [P2] Redact sensitive values from section Debug output — /home/sanya/orca/project/rust/crates/discord-core/src/settings/preloaded.rs:262-263
  Когда вызывающий код форматирует публичные секции напрямую, `NotificationSettings` печатает `notification_center_acked_before_id`, а `LocalizationSettings` печатает locale и timezone name: оба типа используют производный `Debug`. Поэтому проверка `Debug` только для внешнего `PreloadedUserSettings` не предотвращает утечку этих значений в логи через секции; примените redacted `Debug` и к этим типам.
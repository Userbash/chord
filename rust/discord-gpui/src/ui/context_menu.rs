use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::menu::{
    ContextMenu as GpuiContextMenu, ContextMenuExt, PopupMenu, PopupMenuItem,
};

// 1. Четыре обязательных действия контекстного меню из ТЗ: Pin, Copy, Reply, Delete
gpui::actions!(context_menu, [Pin, Copy, Reply, Delete]);

pub struct ContextMenu;

impl ContextMenu {
    /// Построение меню PopupMenu из gpui-kit::component::menu с действиями Pin, Reply, Copy, Delete
    pub fn build_menu(menu: PopupMenu, is_pinned: bool) -> PopupMenu {
        let pin_label = if is_pinned {
            "Открепить сообщение"
        } else {
            "Закрепить сообщение"
        };

        menu.menu(pin_label, Box::new(Pin))
            .menu("Ответить", Box::new(Reply))
            .menu("Копировать текст", Box::new(Copy))
            .separator()
            .menu("Удалить сообщение", Box::new(Delete))
    }

    /// Прикрепление контекстного меню правого клика мыши (ПКМ) к любому элементу интерфейса
    pub fn attach_right_click<E>(element: E, is_pinned: bool) -> GpuiContextMenu<E>
    where
        E: InteractiveElement + ParentElement + Styled,
    {
        element.context_menu(move |menu, _window, _cx| Self::build_menu(menu, is_pinned))
    }

    /// Обработчик действия Pin (Закрепить / Открепить)
    pub fn handle_pin_action(this: &mut AppLayout, cx: &mut Context<AppLayout>) {
        if let Some(msg_id) = this.state.active_context_menu.clone() {
            let ch_id = this.state.selected_channel_id.clone().unwrap_or_default();
            this.state.pin_message(&ch_id, &msg_id);
            this.state.active_context_menu = None;
            cx.notify();
        }
    }

    /// Обработчик действия Copy (Копировать текст сообщения в буфер обмена)
    pub fn handle_copy_action(this: &mut AppLayout, cx: &mut Context<AppLayout>) {
        if let Some(msg_id) = this.state.active_context_menu.clone() {
            let ch_id = this.state.selected_channel_id.clone().unwrap_or_default();
            if let Some(msg) = this
                .state
                .messages
                .get(&ch_id)
                .and_then(|msgs| msgs.iter().find(|m| m.id == msg_id))
            {
                cx.write_to_clipboard(ClipboardItem::new_string(msg.content.clone()));
            }
            this.state.active_context_menu = None;
            cx.notify();
        }
    }

    /// Обработчик действия Reply (Ответить на сообщение: подстановка @Author в инпут)
    pub fn handle_reply_action(this: &mut AppLayout, cx: &mut Context<AppLayout>) {
        if let Some(msg_id) = this.state.active_context_menu.clone() {
            let ch_id = this.state.selected_channel_id.clone().unwrap_or_default();
            if let Some(msg) = this
                .state
                .messages
                .get(&ch_id)
                .and_then(|msgs| msgs.iter().find(|m| m.id == msg_id))
            {
                let author = msg
                    .author
                    .global_name
                    .as_deref()
                    .unwrap_or(&msg.author.username);
                this.state.draft_text = format!("@{}: ", author);
            }
            this.state.active_context_menu = None;
            cx.notify();
        }
    }

    /// Обработчик действия Delete (Удалить сообщение)
    pub fn handle_delete_action(this: &mut AppLayout, cx: &mut Context<AppLayout>) {
        if let Some(msg_id) = this.state.active_context_menu.clone() {
            let ch_id = this.state.selected_channel_id.clone().unwrap_or_default();
            this.state.delete_message(&ch_id, &msg_id);
            this.state.active_context_menu = None;
            cx.notify();
        }
    }

    /// Нативный рендеринг контекстного меню с использованием gpui-kit::component::menu::PopupMenu
    pub fn render(
        state: &AppState,
        window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let msg_id = match &state.active_context_menu {
            Some(id) => id.clone(),
            None => return div().into_any_element(),
        };

        let ch_id = state.selected_channel_id.clone().unwrap_or_default();
        let is_pinned = state
            .messages
            .get(&ch_id)
            .and_then(|msgs| msgs.iter().find(|m| m.id == msg_id))
            .map(|m| m.is_pinned)
            .unwrap_or(false);
        let pin_label = if is_pinned {
            "Открепить сообщение"
        } else {
            "Закрепить сообщение"
        };

        let msg_id_pin = msg_id.clone();
        let ch_id_pin = ch_id.clone();
        let msg_id_reply = msg_id.clone();
        let ch_id_reply = ch_id.clone();
        let msg_id_copy = msg_id.clone();
        let ch_id_copy = ch_id.clone();
        let msg_id_del = msg_id.clone();
        let ch_id_del = ch_id.clone();

        let layout = cx.entity().clone();

        // Построение PopupMenu через gpui-kit::component::menu
        let popup =
            PopupMenu::build(window, cx, move |menu, _window, _cx| {
                let layout_pin = layout.clone();
                let layout_reply = layout.clone();
                let layout_copy = layout.clone();
                let layout_del = layout.clone();

                menu.min_w(px(220.))
                    .item(
                        PopupMenuItem::new(pin_label)
                            .action(Box::new(Pin))
                            .on_click(move |_, _, cx| {
                                layout_pin.update(cx, |this, cx| {
                                    this.state.pin_message(&ch_id_pin, &msg_id_pin);
                                    this.state.active_context_menu = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Ответить")
                            .action(Box::new(Reply))
                            .on_click(move |_, _, cx| {
                                layout_reply.update(cx, |this, cx| {
                                    if let Some(msg) =
                                        this.state.messages.get(&ch_id_reply).and_then(|msgs| {
                                            msgs.iter().find(|m| m.id == msg_id_reply)
                                        })
                                    {
                                        let author = msg
                                            .author
                                            .global_name
                                            .as_deref()
                                            .unwrap_or(&msg.author.username);
                                        this.state.draft_text = format!("@{}: ", author);
                                    }
                                    this.state.active_context_menu = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Копировать текст")
                            .action(Box::new(Copy))
                            .on_click(move |_, _, cx| {
                                layout_copy.update(cx, |this, cx| {
                                    if let Some(msg) =
                                        this.state.messages.get(&ch_id_copy).and_then(|msgs| {
                                            msgs.iter().find(|m| m.id == msg_id_copy)
                                        })
                                    {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            msg.content.clone(),
                                        ));
                                    }
                                    this.state.active_context_menu = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Удалить сообщение")
                            .action(Box::new(Delete))
                            .on_click(move |_, _, cx| {
                                layout_del.update(cx, |this, cx| {
                                    this.state.delete_message(&ch_id_del, &msg_id_del);
                                    this.state.active_context_menu = None;
                                    cx.notify();
                                });
                            }),
                    )
            });

        div()
            .id("context-menu-backdrop")
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000040))
            .flex()
            .items_center()
            .justify_center()
            .on_action(cx.listener(|this, _: &Pin, _, cx| {
                Self::handle_pin_action(this, cx);
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                Self::handle_copy_action(this, cx);
            }))
            .on_action(cx.listener(|this, _: &Reply, _, cx| {
                Self::handle_reply_action(this, cx);
            }))
            .on_action(cx.listener(|this, _: &Delete, _, cx| {
                Self::handle_delete_action(this, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.state.active_context_menu = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(220.))
                    .bg(DISCORD_THEME.bg_tertiary)
                    .rounded(px(8.))
                    .border_1()
                    .border_color(DISCORD_THEME.border_strong)
                    .p(px(2.))
                    .child(popup),
            )
            .into_any_element()
    }
}

/// Трейт расширения для добавления ПКМ контекстного меню сообщений к любому элементу
pub trait RightClickContextMenuExt: InteractiveElement + ParentElement + Styled + Sized {
    fn right_click_context_menu(self, is_pinned: bool) -> GpuiContextMenu<Self> {
        ContextMenu::attach_right_click(self, is_pinned)
    }
}

impl<E: InteractiveElement + ParentElement + Styled + Sized> RightClickContextMenuExt for E {}

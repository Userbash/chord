#!/usr/bin/env bash
# Мутационный «спайк»: корректные маркеры и snapshot, но окно умирает сразу (P2-3).
echo "discord-gui-spike: window-ready"
echo 'UI_SNAPSHOT {"screenCount":4,"chatRows":100000,"memberRows":24229,"emojiCategories":10,"settingsToggles":38,"chatRenderedRows":30,"memberRenderedRows":24,"chatLoadedRows":400,"memberLoadedRows":100,"actions":{"screenClicks":4,"categoryClicks":true,"toggleClick":true,"listScroll":true}}'
sleep 1
exit 0

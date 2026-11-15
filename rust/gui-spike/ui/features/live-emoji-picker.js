// @ts-check
/**
 * Live Discord emoji picker popout and quick-reaction toggle controller.
 * Extracted from `app.js` as part of RW51-EPIC2-T4.
 * @module ui/features/live-emoji-picker
 */

/** @type {Array<{ id: string, icon: string, title: string, items: Array<{ glyph: string, shortcode: string, name: string }> }>} */
export const EMOJI_PICKER_CATEGORIES = [
  {
    id: "recent",
    icon: "🕒",
    title: "НЕДАВНО ИСПОЛЬЗОВАННЫЕ",
    items: [
      { glyph: "👋", shortcode: ":wave:", name: "привет машет рукой" },
      { glyph: "👍", shortcode: ":thumbsup:", name: "класс палец вверх" },
      { glyph: "🔥", shortcode: ":fire:", name: "огонь" },
      { glyph: "👀", shortcode: ":eyes:", name: "глаза" },
      { glyph: "❤️", shortcode: ":heart:", name: "сердце любовь" },
      { glyph: "😂", shortcode: ":joy:", name: "смех до слез" },
      { glyph: "🎉", shortcode: ":tada:", name: "праздник хлопушка" },
      { glyph: "💯", shortcode: ":100:", name: "сто процентов" },
    ],
  },
  {
    id: "people",
    icon: "😀",
    title: "ЛЮДИ И ЭМОЦИИ",
    items: [
      { glyph: "😀", shortcode: ":grinning:", name: "улыбка радость" },
      { glyph: "😃", shortcode: ":smiley:", name: "широкая улыбка" },
      { glyph: "😄", shortcode: ":smile:", name: "смеющаяся улыбка" },
      { glyph: "😁", shortcode: ":grin:", name: "сияющая улыбка" },
      { glyph: "😆", shortcode: ":laughing:", name: "смех зажмурившись" },
      { glyph: "🥹", shortcode: ":holding_back_tears:", name: "умиление слезы" },
      { glyph: "😅", shortcode: ":sweat_smile:", name: "нервная улыбка" },
      { glyph: "😂", shortcode: ":joy:", name: "смех до слез" },
      { glyph: "🤣", shortcode: ":rofl:", name: "катаюсь от смеха" },
      { glyph: "🥲", shortcode: ":smiling_face_with_tear:", name: "улыбка со слезой" },
      { glyph: "☺️", shortcode: ":relaxed:", name: "довольное лицо" },
      { glyph: "😊", shortcode: ":blush:", name: "застенчивая улыбка" },
      { glyph: "😇", shortcode: ":innocent:", name: "ангел нимб" },
      { glyph: "🙂", shortcode: ":slight_smile:", name: "легкая улыбка" },
      { glyph: "🙃", shortcode: ":upside_down:", name: "перевернутое лицо" },
      { glyph: "😉", shortcode: ":wink:", name: "подмигивание" },
      { glyph: "😌", shortcode: ":relieved:", name: "облегчение" },
      { glyph: "😍", shortcode: ":heart_eyes:", name: "влюбленный глаза сердечки" },
      { glyph: "🥰", shortcode: ":smiling_face_with_3_hearts:", name: "в сердечках любовь" },
      { glyph: "😘", shortcode: ":kissing_heart:", name: "воздушный поцелуй" },
      { glyph: "😎", shortcode: ":sunglasses:", name: "крутой в очках" },
      { glyph: "🤩", shortcode: ":star_struck:", name: "восторг звезды в глазах" },
      { glyph: "🥳", shortcode: ":partying_face:", name: "вечеринка праздник" },
      { glyph: "😏", shortcode: ":smirk:", name: "усмешка" },
      { glyph: "🤔", shortcode: ":thinking:", name: "задумчивый думает" },
      { glyph: "🫡", shortcode: ":saluting_face:", name: "честь салют так точно" },
      { glyph: "😭", shortcode: ":sob:", name: "рыдания плач" },
      { glyph: "🤯", shortcode: ":exploding_head:", name: "взрыв мозга шок" },
      { glyph: "🙏", shortcode: ":pray:", name: "спасибо пожалуйста молитва" },
      { glyph: "👏", shortcode: ":clap:", name: "аплодисменты хлопки" },
      { glyph: "🤝", shortcode: ":handshake:", name: "рукопожатие договор" },
      { glyph: "💪", shortcode: ":muscle:", name: "сила бицепс" },
    ],
  },
  {
    id: "nature",
    icon: "🐻",
    title: "ЖИВОТНЫЕ И ПРИРОДА",
    items: [
      { glyph: "🐶", shortcode: ":dog:", name: "собака песик" },
      { glyph: "🐱", shortcode: ":cat:", name: "кот кошка" },
      { glyph: "🐭", shortcode: ":mouse:", name: "мышь" },
      { glyph: "🐹", shortcode: ":hamster:", name: "хомяк" },
      { glyph: "🐰", shortcode: ":rabbit:", name: "кролик заяц" },
      { glyph: "🦊", shortcode: ":fox:", name: "лиса" },
      { glyph: "🐻", shortcode: ":bear:", name: "медведь" },
      { glyph: "🐼", shortcode: ":panda_face:", name: "панда" },
      { glyph: "🦁", shortcode: ":lion:", name: "лев" },
      { glyph: "🐸", shortcode: ":frog:", name: "лягушка пепе" },
      { glyph: "🐵", shortcode: ":monkey_face:", name: "обезьяна" },
      { glyph: "🦄", shortcode: ":unicorn:", name: "единорог" },
      { glyph: "🐝", shortcode: ":bee:", name: "пчела" },
      { glyph: "🦋", shortcode: ":butterfly:", name: "бабочка" },
      { glyph: "🌸", shortcode: ":cherry_blossom:", name: "сакура цветок" },
      { glyph: "🌹", shortcode: ":rose:", name: "роза цветок" },
    ],
  },
  {
    id: "food",
    icon: "🍔",
    title: "ЕДА И НАПИТКИ",
    items: [
      { glyph: "🍎", shortcode: ":apple:", name: "яблоко" },
      { glyph: "🍕", shortcode: ":pizza:", name: "пицца" },
      { glyph: "🍔", shortcode: ":hamburger:", name: "бургер гамбургер" },
      { glyph: "🍟", shortcode: ":fries:", name: "картошка фри" },
      { glyph: "🌭", shortcode: ":hotdog:", name: "хотдог" },
      { glyph: "🍿", shortcode: ":popcorn:", name: "попкорн" },
      { glyph: "🍩", shortcode: ":doughnut:", name: "пончик" },
      { glyph: "🍪", shortcode: ":cookie:", name: "печенье" },
      { glyph: "🎂", shortcode: ":birthday:", name: "торт день рождения" },
      { glyph: "🍰", shortcode: ":cake:", name: "пирожное тортик" },
      { glyph: "🍫", shortcode: ":chocolate_bar:", name: "шоколад" },
      { glyph: "☕", shortcode: ":coffee:", name: "кофе чай" },
      { glyph: "🍵", shortcode: ":tea:", name: "зеленый чай" },
      { glyph: "🥤", shortcode: ":cup_with_straw:", name: "газировка напиток" },
      { glyph: "🍺", shortcode: ":beer:", name: "пиво кружка" },
      { glyph: "🥂", shortcode: ":clinking_glasses:", name: "шампанское бокалы тост" },
    ],
  },
  {
    id: "activity",
    icon: "⚽",
    title: "АКТИВНОСТИ",
    items: [
      { glyph: "⚽", shortcode: ":soccer:", name: "футбол мяч" },
      { glyph: "🏀", shortcode: ":basketball:", name: "баскетбол" },
      { glyph: "🎮", shortcode: ":video_game:", name: "геймпад игры" },
      { glyph: "🕹️", shortcode: ":joystick:", name: "джойстик аркада" },
      { glyph: "🎲", shortcode: ":game_die:", name: "кубик кости" },
      { glyph: "🎯", shortcode: ":dart:", name: "мишень точно в цель" },
      { glyph: "🏆", shortcode: ":trophy:", name: "кубок трофей победа" },
      { glyph: "🥇", shortcode: ":first_place:", name: "золотая медаль первое место" },
      { glyph: "🎸", shortcode: ":guitar:", name: "гитара музыка" },
      { glyph: "🎧", shortcode: ":headphones:", name: "наушники звук" },
      { glyph: "🎤", shortcode: ":microphone:", name: "микрофон вокал" },
      { glyph: "🎬", shortcode: ":clapper:", name: "кино хлопушка" },
      { glyph: "🎨", shortcode: ":art:", name: "палитра творчество" },
      { glyph: "🎭", shortcode: ":performing_arts:", name: "театр маски" },
      { glyph: "🎪", shortcode: ":circus_tent:", name: "цирк шатер" },
      { glyph: "🚀", shortcode: ":rocket:", name: "ракета запуск космос" },
    ],
  },
  {
    id: "travel",
    icon: "🚗",
    title: "ПУТЕШЕСТВИЯ И МЕСТА",
    items: [
      { glyph: "🚗", shortcode: ":red_car:", name: "машина автомобиль" },
      { glyph: "🚕", shortcode: ":taxi:", name: "такси" },
      { glyph: "🚌", shortcode: ":bus:", name: "автобус" },
      { glyph: "🏎️", shortcode: ":racing_car:", name: "гоночный болид" },
      { glyph: "✈️", shortcode: ":airplane:", name: "самолет полет" },
      { glyph: "🛸", shortcode: ":flying_saucer:", name: "нло тарелка" },
      { glyph: "⛵", shortcode: ":boat:", name: "парусник лодка" },
      { glyph: "🏠", shortcode: ":house:", name: "дом здание" },
      { glyph: "🏰", shortcode: ":castle:", name: "замок крепость" },
      { glyph: "🌍", shortcode: ":earth_africa:", name: "земля глобус мир" },
      { glyph: "🌙", shortcode: ":crescent_moon:", name: "луна месяц ночь" },
      { glyph: "⭐", shortcode: ":star:", name: "звезда" },
      { glyph: "🌟", shortcode: ":star2:", name: "сияющая звезда" },
      { glyph: "⚡", shortcode: ":zap:", name: "молния разряд" },
      { glyph: "🔥", shortcode: ":fire:", name: "огонь пламя" },
      { glyph: "🌈", shortcode: ":rainbow:", name: "радуга" },
    ],
  },
  {
    id: "objects",
    icon: "💡",
    title: "ПРЕДМЕТЫ",
    items: [
      { glyph: "💡", shortcode: ":bulb:", name: "лампочка идея" },
      { glyph: "💻", shortcode: ":computer:", name: "ноутбук компьютер код" },
      { glyph: "🖥️", shortcode: ":desktop:", name: "монитор пк" },
      { glyph: "⌨️", shortcode: ":keyboard:", name: "клавиатура" },
      { glyph: "📱", shortcode: ":iphone:", name: "телефон смартфон" },
      { glyph: "📷", shortcode: ":camera:", name: "камера фото" },
      { glyph: "🔔", shortcode: ":bell:", name: "колокольчик уведомление" },
      { glyph: "📢", shortcode: ":loudspeaker:", name: "рупор объявление" },
      { glyph: "💎", shortcode: ":gem:", name: "алмаз бриллиант" },
      { glyph: "🔑", shortcode: ":key:", name: "ключ доступ" },
      { glyph: "🔒", shortcode: ":lock:", name: "замок закрыто безопасность" },
      { glyph: "🔓", shortcode: ":unlock:", name: "открытый замок" },
      { glyph: "🛠️", shortcode: ":tools:", name: "инструменты настройка" },
      { glyph: "⚙️", shortcode: ":gear:", name: "шестеренка настройки" },
      { glyph: "📦", shortcode: ":package:", name: "коробка пакет релиз" },
      { glyph: "🎁", shortcode: ":gift:", name: "подарок сюрприз" },
    ],
  },
  {
    id: "symbols",
    icon: "🔣",
    title: "СИМВОЛЫ",
    items: [
      { glyph: "❤️", shortcode: ":heart:", name: "красное сердце" },
      { glyph: "🧡", shortcode: ":orange_heart:", name: "оранжевое сердце" },
      { glyph: "💛", shortcode: ":yellow_heart:", name: "желтое сердце" },
      { glyph: "💚", shortcode: ":green_heart:", name: "зеленое сердце" },
      { glyph: "💙", shortcode: ":blue_heart:", name: "синее сердце" },
      { glyph: "💜", shortcode: ":purple_heart:", name: "фиолетовое сердце" },
      { glyph: "🖤", shortcode: ":black_heart:", name: "черное сердце" },
      { glyph: "🤍", shortcode: ":white_heart:", name: "белое сердце" },
      { glyph: "💔", shortcode: ":broken_heart:", name: "разбитое сердце" },
      { glyph: "✅", shortcode: ":white_check_mark:", name: "галочка выполнено ок" },
      { glyph: "☑️", shortcode: ":ballot_box_with_check:", name: "чекбокс галочка" },
      { glyph: "❌", shortcode: ":x:", name: "крестик ошибка отмена" },
      { glyph: "⚠️", shortcode: ":warning:", name: "внимание предупреждение" },
      { glyph: "❗", shortcode: ":exclamation:", name: "восклицательный знак важно" },
      { glyph: "❓", shortcode: ":question:", name: "вопросительный знак вопрос" },
      { glyph: "💯", shortcode: ":100:", name: "сто баллов" },
    ],
  },
  {
    id: "flags",
    icon: "🏳️",
    title: "ФЛАГИ",
    items: [
      { glyph: "🏁", shortcode: ":checkered_flag:", name: "финишный флаг гонка" },
      { glyph: "🚩", shortcode: ":triangular_flag_on_post:", name: "красный флажок отметка" },
      { glyph: "🏳️", shortcode: ":white_flag:", name: "белый флаг" },
      { glyph: "🏴", shortcode: ":black_flag:", name: "черный флаг" },
      { glyph: "🇷🇺", shortcode: ":flag_ru:", name: "флаг россия ru" },
      { glyph: "🇺🇸", shortcode: ":flag_us:", name: "флаг сша us" },
      { glyph: "🇬🇧", shortcode: ":flag_gb:", name: "флаг великобритания gb" },
      { glyph: "🇩🇪", shortcode: ":flag_de:", name: "флаг германия de" },
      { glyph: "🇫🇷", shortcode: ":flag_fr:", name: "флаг франция fr" },
      { glyph: "🇯🇵", shortcode: ":flag_jp:", name: "флаг япония jp" },
      { glyph: "🇨🇳", shortcode: ":flag_cn:", name: "флаг китай cn" },
      { glyph: "🇰🇿", shortcode: ":flag_kz:", name: "флаг казахстан kz" },
      { glyph: "🇧🇾", shortcode: ":flag_by:", name: "флаг беларусь by" },
      { glyph: "🇧🇷", shortcode: ":flag_br:", name: "флаг бразилия br" },
      { glyph: "🇮🇳", shortcode: ":flag_in:", name: "флаг индия in" },
      { glyph: "🇰🇷", shortcode: ":flag_kr:", name: "флаг корея kr" },
    ],
  },
];

/**
 * Promote or insert an emoji into the bounded LRU recent-reactions list (max 8 entries).
 *
 * @param {Array<{ glyph: string, shortcode: string, name: string }>} recentList
 * @param {string} glyph
 * @param {string} shortcode
 * @param {string} [name]
 */
function recordRecentEmoji(recentList, glyph, shortcode, name = shortcode) {
  const existingIdx = recentList.findIndex((e) => e.glyph === glyph);
  if (existingIdx > 0) {
    const [picked] = recentList.splice(existingIdx, 1);
    if (picked) recentList.unshift(picked);
  } else if (existingIdx < 0) {
    recentList.unshift({ glyph, shortcode, name });
    if (recentList.length > 8) recentList.pop();
  }
}

/**
 * Create the live emoji picker and quick-reaction controller.
 *
 * @param {any} ctx
 */
export function createLiveEmojiPickerController(ctx) {
  const { byId, chat, composer, invoke } = ctx;

  const recentReactionEmojis = [
    { glyph: "👍", shortcode: ":thumbsup:", name: "класс палец вверх" },
    { glyph: "👋", shortcode: ":wave:", name: "привет машет рукой" },
    { glyph: "🔥", shortcode: ":fire:", name: "огонь" },
    { glyph: "👀", shortcode: ":eyes:", name: "глаза" },
  ];

  /**
   * Toggle a quick reaction on a message row in the DOM and persist via `toggle_reaction` IPC.
   *
   * @param {HTMLElement} msgRow
   * @param {string} emojiGlyph
   * @param {string} [shortcode]
   */
  function applyQuickReactionToRow(msgRow, emojiGlyph, shortcode = ":emoji:") {
    recordRecentEmoji(recentReactionEmojis, emojiGlyph, shortcode, shortcode);
    const msgId = String(
      msgRow.getAttribute("data-message-id") || msgRow.getAttribute("data-id") || "",
    ).trim();
    const chId = String(
      msgRow.getAttribute("data-channel-id") || chat.channelId || "",
    ).trim();
    const textEl = msgRow.querySelector(".text") || msgRow;
    let group = /** @type {HTMLElement | null} */ (msgRow.querySelector(".reactions"));
    if (!group) {
      group = document.createElement("div");
      group.className = "reactions";
      group.setAttribute("role", "group");
      group.setAttribute("aria-label", "Реакции");
      textEl.append(group);
    }
    let pill = /** @type {HTMLElement | null} */ (
      group.querySelector(`.reaction-pill[data-emoji="${emojiGlyph}"]`)
    );
    let nextActive = true;
    if (pill) {
      const active = pill.getAttribute("data-me") === "true";
      const countEl = pill.querySelector(".reaction-count");
      const curCount = Number(countEl?.textContent || "1") || 1;
      if (active) {
        nextActive = false;
        if (curCount <= 1) {
          pill.remove();
        } else {
          pill.setAttribute("data-me", "false");
          pill.setAttribute("aria-pressed", "false");
          if (countEl) countEl.textContent = String(curCount - 1);
        }
      } else {
        nextActive = true;
        pill.setAttribute("data-me", "true");
        pill.setAttribute("aria-pressed", "true");
        if (countEl) countEl.textContent = String(curCount + 1);
      }
    } else {
      nextActive = true;
      const newPill = document.createElement("button");
      newPill.type = "button";
      newPill.className = "reaction-pill";
      newPill.setAttribute("data-emoji", emojiGlyph);
      newPill.setAttribute("data-me", "true");
      newPill.setAttribute("aria-pressed", "true");
      newPill.innerHTML = `<span class="reaction-emoji">${emojiGlyph}</span><span class="reaction-count">1</span>`;
      newPill.addEventListener("click", () =>
        applyQuickReactionToRow(msgRow, emojiGlyph, shortcode),
      );
      group.append(newPill);
    }
    if (chId && msgId) {
      void invoke("toggle_reaction", {
        channelId: chId,
        messageId: msgId,
        emoji: emojiGlyph,
        active: nextActive,
      }).catch(() => {});
    }
  }

  /**
   * Mount the shared Discord emoji picker popout (`#live-emoji-picker-popout`)
   * for both composer emoji insertion and message reaction selection.
   *
   * @param {{
   *   left: number,
   *   top: number,
   *   ariaLabel: string,
   *   onSelect: (glyph: string, shortcode: string) => void
   * }} opts
   */
  function mountLiveDiscordEmojiPickerPopout(opts) {
    document.getElementById("live-emoji-picker-popout")?.remove();
    const pop = document.createElement("div");
    pop.id = "live-emoji-picker-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", opts.ariaLabel);
    const winW = window.innerWidth || 1280;
    const winH = window.innerHeight || 800;
    const left = Math.max(16, Math.min(winW - 416, opts.left));
    const top = Math.max(16, Math.min(winH - 446, opts.top));
    pop.style.cssText =
      `position:fixed;left:${left}px;top:${top}px;width:400px;height:430px;background:#2b2d31;color:#f2f3f5;border:1px solid #1e1f22;border-radius:10px;box-shadow:0 16px 40px rgba(0,0,0,0.75);z-index:10050;display:flex;flex-direction:column;overflow:hidden;font-family:inherit;`;

    const topSearch = document.createElement("div");
    topSearch.style.cssText =
      "padding:10px 12px;border-bottom:1px solid #1e1f22;display:flex;align-items:center;gap:8px;background:#232428;";
    const searchInp = document.createElement("input");
    searchInp.type = "search";
    searchInp.placeholder = "Найти идеальный эмодзи";
    searchInp.setAttribute("aria-label", "Поиск эмодзи");
    searchInp.style.cssText =
      "flex:1;padding:7px 10px;border-radius:6px;border:1px solid #1e1f22;background:#1e1f22;color:#f2f3f5;font-size:13px;outline:none;";
    const SKIN_TONES = ["👏", "👏🏻", "👏🏼", "👏🏽", "👏🏾", "👏🏿"];
    let toneIdx = 0;
    const clapBadge = document.createElement("button");
    clapBadge.type = "button";
    clapBadge.textContent = SKIN_TONES[0] ?? "👏";
    clapBadge.title = "Тон кожи эмодзи";
    clapBadge.style.cssText =
      "font-size:18px;cursor:pointer;background:transparent;border:none;padding:4px;border-radius:6px;line-height:1;";
    clapBadge.addEventListener("click", () => {
      toneIdx = (toneIdx + 1) % SKIN_TONES.length;
      clapBadge.textContent = SKIN_TONES[toneIdx] || "👏";
    });
    topSearch.append(searchInp, clapBadge);

    const bodyWrap = document.createElement("div");
    bodyWrap.style.cssText = "flex:1;display:flex;min-height:0;";

    const catRail = document.createElement("div");
    catRail.style.cssText =
      "width:44px;background:#1e1f22;display:flex;flex-direction:column;align-items:center;padding:8px 0;gap:6px;font-size:17px;color:#949ba4;flex-shrink:0;overflow-y:auto;";

    const scrollArea = document.createElement("div");
    scrollArea.style.cssText =
      "flex:1;overflow-y:auto;padding:10px 12px;display:flex;flex-direction:column;gap:14px;scroll-behavior:smooth;";

    const footerBar = document.createElement("div");
    footerBar.style.cssText =
      "height:42px;background:#232428;border-top:1px solid #1e1f22;padding:0 12px;display:flex;align-items:center;justify-content:space-between;font-size:13px;font-weight:600;color:#dbdee1;";
    footerBar.innerHTML = `<span style="display:flex;align-items:center;gap:8px;"><span style="font-size:20px;">😀</span><span>:grinning:</span></span><span style="font-size:11px;color:#949ba4;">Эмодзи</span>`;

    /** @type {Map<string, HTMLElement>} */
    const sectionMap = new Map();
    /** @type {Map<string, HTMLButtonElement>} */
    const catBtnMap = new Map();
    let activeCatId = "recent";

    const updateActiveCatButton = (/** @type {string} */ catId) => {
      activeCatId = catId;
      for (const [id, btn] of catBtnMap.entries()) {
        btn.style.background = id === catId ? "#35373c" : "transparent";
        btn.style.color = id === catId ? "#f2f3f5" : "#949ba4";
      }
    };

    for (const cat of EMOJI_PICKER_CATEGORIES) {
      const cBtn = document.createElement("button");
      cBtn.type = "button";
      cBtn.textContent = cat.icon;
      cBtn.title = cat.title;
      cBtn.setAttribute("aria-label", cat.title);
      cBtn.style.cssText =
        "cursor:pointer;padding:5px;border-radius:6px;border:none;background:transparent;font-size:17px;line-height:1;width:32px;height:32px;display:flex;align-items:center;justify-content:center;";
      cBtn.addEventListener("click", () => {
        updateActiveCatButton(cat.id);
        if (searchInp.value.trim() !== "") {
          searchInp.value = "";
          renderGrid("");
        }
        const sec = sectionMap.get(cat.id);
        if (sec) {
          sec.scrollIntoView({ block: "start", behavior: "smooth" });
        }
      });
      catBtnMap.set(cat.id, cBtn);
      catRail.append(cBtn);
    }
    updateActiveCatButton(activeCatId);

    const renderGrid = (/** @type {string} */ q) => {
      scrollArea.replaceChildren();
      sectionMap.clear();
      const clean = q.trim().toLowerCase();
      const dynamicCats = [
        {
          id: "recent",
          icon: "🕒",
          title: "НЕДАВНО ИСПОЛЬЗОВАННЫЕ",
          items: [
            ...recentReactionEmojis,
            ...(EMOJI_PICKER_CATEGORIES[0]?.items ?? []).filter(
              (x) => !recentReactionEmojis.some((r) => r.glyph === x.glyph),
            ),
          ].slice(0, 8),
        },
        ...EMOJI_PICKER_CATEGORIES.slice(1),
      ];
      let totalMatches = 0;
      for (const cat of dynamicCats) {
        const filtered = cat.items.filter(
          (it) =>
            !clean ||
            it.shortcode.toLowerCase().includes(clean) ||
            (it.name && it.name.toLowerCase().includes(clean)) ||
            it.glyph.includes(clean),
        );
        if (filtered.length === 0) continue;
        totalMatches += filtered.length;
        const sec = document.createElement("div");
        sec.setAttribute("data-emoji-cat-id", cat.id);
        const h = document.createElement("div");
        h.style.cssText =
          "font-size:11px;font-weight:800;color:#949ba4;margin-bottom:6px;letter-spacing:0.04em;display:flex;align-items:center;gap:6px;";
        h.textContent = `${cat.icon} ${cat.title}`;
        const grid = document.createElement("div");
        grid.style.cssText = "display:grid;grid-template-columns:repeat(8,1fr);gap:4px;";
        for (const item of filtered) {
          const eb = document.createElement("button");
          eb.type = "button";
          eb.textContent = item.glyph;
          eb.title = item.shortcode;
          eb.setAttribute("data-emoji-glyph", item.glyph);
          eb.style.cssText =
            "height:36px;border:none;border-radius:6px;background:transparent;font-size:22px;cursor:pointer;display:flex;align-items:center;justify-content:center;";
          eb.addEventListener("mouseenter", () => {
            eb.style.background = "#35373c";
            footerBar.innerHTML = `<span style="display:flex;align-items:center;gap:8px;"><span style="font-size:20px;">${item.glyph}</span><span>${item.shortcode}</span></span><span style="font-size:11px;color:#949ba4;">${cat.title}</span>`;
          });
          eb.addEventListener("mouseleave", () => {
            eb.style.background = "transparent";
          });
          eb.addEventListener("click", () => {
            recordRecentEmoji(
              recentReactionEmojis,
              item.glyph,
              item.shortcode,
              item.name || item.shortcode,
            );
            opts.onSelect(item.glyph, item.shortcode);
            pop.remove();
            document.getElementById("live-msg-context-menu")?.remove();
          });
          grid.append(eb);
        }
        sec.append(h, grid);
        sectionMap.set(cat.id, sec);
        scrollArea.append(sec);
      }
      if (totalMatches === 0) {
        const emptyEl = document.createElement("div");
        emptyEl.style.cssText =
          "padding:36px 16px;text-align:center;color:#949ba4;font-size:13px;";
        emptyEl.textContent = "Эмодзи по запросу не найдены";
        scrollArea.append(emptyEl);
      }
    };
    renderGrid("");
    searchInp.addEventListener("input", () => renderGrid(searchInp.value));

    bodyWrap.append(catRail, scrollArea);
    pop.append(topSearch, bodyWrap, footerBar);
    document.body.append(pop);
    requestAnimationFrame(() => searchInp.focus());
  }

  /**
   * Open the emoji picker popout anchored to a message reaction trigger.
   *
   * @param {HTMLElement} msgRow
   * @param {number} clientX
   * @param {number} clientY
   */
  function openLiveMessageEmojiPicker(msgRow, clientX, clientY) {
    mountLiveDiscordEmojiPickerPopout({
      left: clientX,
      top: clientY,
      ariaLabel: "Выбор эмодзи для реакции",
      onSelect: (glyph, shortcode) => {
        applyQuickReactionToRow(msgRow, glyph, shortcode);
      },
    });
  }

  /**
   * Open the emoji picker popout above the message composer input.
   *
   * @param {HTMLElement | null} [anchorEl]
   */
  function openLiveComposerEmojiPicker(anchorEl) {
    const winW = window.innerWidth || 1280;
    const winH = window.innerHeight || 800;
    const rect = anchorEl?.getBoundingClientRect?.();
    const left = rect ? Math.max(16, Math.min(winW - 416, rect.right - 400)) : winW - 440;
    const top = rect ? Math.max(16, Math.min(winH - 442, rect.top - 442)) : winH - 510;
    mountLiveDiscordEmojiPickerPopout({
      left,
      top,
      ariaLabel: "Выбор эмодзи для сообщения",
      onSelect: (glyph) => {
        composer.insertText(glyph);
        const inp = /** @type {HTMLTextAreaElement | HTMLInputElement | null} */ (
          byId("composer-input")
        );
        inp?.focus?.();
      },
    });
  }

  /**
   * Return the current list of recent reaction emojis.
   */
  function getRecentReactionEmojis() {
    return recentReactionEmojis;
  }

  return {
    applyQuickReactionToRow,
    mountLiveDiscordEmojiPickerPopout,
    openLiveMessageEmojiPicker,
    openLiveComposerEmojiPicker,
    getRecentReactionEmojis,
  };
}

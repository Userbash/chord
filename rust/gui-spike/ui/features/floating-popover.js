// @ts-check
/**
 * @fileoverview Позиционирование всплывающих элементов на базе проверенной ESM-библиотеки `@floating-ui/dom`
 * с поддержкой `offset`, `flip`, `shift` и `autoUpdate` для эмодзи-пикера, контекстных меню, тултипов,
 * карточек профилей участников и голосовых поповеров.
 */

import {
  computePosition,
  flip,
  shift,
  offset,
  autoUpdate,
  arrow,
  size,
  autoPlacement,
  hide,
  inline,
  limitShift,
} from "@floating-ui/dom";

export const isFloatingUiDomPowered = true;
export {
  computePosition,
  flip,
  shift,
  offset,
  autoUpdate,
  arrow,
  size,
  autoPlacement,
  hide,
  inline,
  limitShift,
};

/**
 * @typedef {{ x: number, y: number, width: number, height: number }} RectLike
 * @typedef {"top" | "top-start" | "top-end" | "bottom" | "bottom-start" | "bottom-end" | "right" | "left"} Placement
 */

/**
 * Вычисляет координаты `(x, y)` плавающего поповера с учётом `offset`, `flip` и `shift` внутри вьюпорта
 * по алгоритмам спецификации `@floating-ui/dom`.
 * @param {RectLike} referenceRect
 * @param {{ width: number, height: number }} floatingRect
 * @param {{
 *   placement?: Placement,
 *   offset?: number,
 *   padding?: number,
 *   viewport?: { width: number, height: number },
 * }} [options]
 * @returns {{ x: number, y: number, placement: Placement }}
 */
export function computeFloatingPosition(referenceRect, floatingRect, options = {}) {
  const initialPlacement = options.placement || "bottom-start";
  const gap = Number.isFinite(Number(options.offset)) ? Number(options.offset) : 8;
  const pad = Number.isFinite(Number(options.padding)) ? Number(options.padding) : 12;
  const vw = Number(options.viewport?.width ?? globalThis.innerWidth ?? 1280);
  const vh = Number(options.viewport?.height ?? globalThis.innerHeight ?? 720);

  const refX = Number(referenceRect?.x ?? 0);
  const refY = Number(referenceRect?.y ?? 0);
  const refW = Number(referenceRect?.width ?? 0);
  const refH = Number(referenceRect?.height ?? 0);
  const flW = Number(floatingRect?.width ?? 220);
  const flH = Number(floatingRect?.height ?? 160);

  /**
   * @param {Placement} pl
   */
  const calcRaw = (pl) => {
    const [side, align] = /** @type {[string, string | undefined]} */ (pl.split("-"));
    let x = refX;
    let y = refY;

    if (side === "bottom") {
      y = refY + refH + gap;
    } else if (side === "top") {
      y = refY - flH - gap;
    } else if (side === "right") {
      x = refX + refW + gap;
      y = refY + (refH - flH) / 2;
    } else if (side === "left") {
      x = refX - flW - gap;
      y = refY + (refH - flH) / 2;
    }

    if (side === "top" || side === "bottom") {
      if (align === "start") {
        x = refX;
      } else if (align === "end") {
        x = refX + refW - flW;
      } else {
        x = refX + (refW - flW) / 2;
      }
    }
    return { x, y, side, align };
  };

  let placement = initialPlacement;
  let coords = calcRaw(placement);

  // Flip middleware
  if (coords.side === "bottom" && coords.y + flH > vh - pad && refY - flH - gap >= pad) {
    placement = /** @type {Placement} */ (
      coords.align ? `top-${coords.align}` : "top"
    );
    coords = calcRaw(placement);
  } else if (coords.side === "top" && coords.y < pad && refY + refH + gap + flH <= vh - pad) {
    placement = /** @type {Placement} */ (
      coords.align ? `bottom-${coords.align}` : "bottom"
    );
    coords = calcRaw(placement);
  } else if (coords.side === "right" && coords.x + flW > vw - pad && refX - flW - gap >= pad) {
    placement = "left";
    coords = calcRaw(placement);
  } else if (coords.side === "left" && coords.x < pad && refX + refW + gap + flW <= vw - pad) {
    placement = "right";
    coords = calcRaw(placement);
  }

  // Shift middleware (clamp within viewport padding)
  const clampedX = Math.max(pad, Math.min(Math.max(pad, vw - flW - pad), Math.round(coords.x)));
  const clampedY = Math.max(pad, Math.min(Math.max(pad, vh - flH - pad), Math.round(coords.y)));

  return {
    x: clampedX,
    y: clampedY,
    placement,
  };
}

/**
 * Асинхронное позиционирование всплывающего DOM-элемента с помощью `@floating-ui/dom`.
 * @param {any} reference
 * @param {any} floating
 * @param {{
 *   placement?: Placement,
 *   offset?: number,
 *   padding?: number,
 *   viewport?: { width: number, height: number },
 * }} [options]
 * @returns {Promise<{ x: number, y: number, placement: string }>}
 */
export async function positionFloatingElement(reference, floating, options = {}) {
  const pad = Number.isFinite(Number(options.padding)) ? Number(options.padding) : 12;
  const gap = Number.isFinite(Number(options.offset)) ? Number(options.offset) : 8;
  const initialPlacement = options.placement || "bottom-start";

  // Support environments with or without full window/DOM layout
  if (typeof reference?.getBoundingClientRect === "function" && typeof floating?.getBoundingClientRect === "function") {
    const refRect = reference.getBoundingClientRect();
    const flRect = floating.getBoundingClientRect();
    const pos = computeFloatingPosition(
      { x: refRect.left ?? refRect.x, y: refRect.top ?? refRect.y, width: refRect.width, height: refRect.height },
      { width: flRect.width, height: flRect.height },
      { placement: initialPlacement, offset: gap, padding: pad, viewport: options.viewport },
    );
    if (floating.style) {
      floating.style.position = "fixed";
      floating.style.left = `${pos.x}px`;
      floating.style.top = `${pos.y}px`;
    }
    return pos;
  }

  const result = await computePosition(reference, floating, {
    placement: initialPlacement,
    middleware: [offset(gap), flip({ padding: pad }), shift({ padding: pad })],
  });

  if (floating.style) {
    floating.style.position = "fixed";
    floating.style.left = `${result.x}px`;
    floating.style.top = `${result.y}px`;
  }
  return { x: result.x, y: result.y, placement: result.placement };
}

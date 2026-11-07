// @ts-check
/**
 * TASK-616 (RW-40 / G2): Alias / Re-export для stream-viewer-ui.
 * Обеспечивает полную обратную совместимость импортов stream-viewer-ui и stream-viewer.
 *
 * @module ui/features/stream-viewer-ui
 */

export {
  STREAM_VIEWER_MODES,
  STREAM_VIEWER_ZOOM_LIMITS,
  DOCUMENT_PIP_TRIGGER_ID,
  clampStreamZoomScale,
  formatStreamVolumeLabel,
  createStreamViewerController,
} from './stream-viewer.js';

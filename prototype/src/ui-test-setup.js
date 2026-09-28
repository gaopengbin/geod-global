import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

afterEach(cleanup);

// jsdom has no layout/pointer capture. Interaction assertions below still use
// actual Radix event handling and focus; browser QA covers positioning/layout.
if (!window.PointerEvent) window.PointerEvent = window.MouseEvent;
if (!window.ResizeObserver) {
  window.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} };
}
if (!HTMLElement.prototype.scrollIntoView) HTMLElement.prototype.scrollIntoView = function () {};
if (!HTMLElement.prototype.hasPointerCapture) HTMLElement.prototype.hasPointerCapture = () => false;
if (!HTMLElement.prototype.setPointerCapture) HTMLElement.prototype.setPointerCapture = function () {};
if (!HTMLElement.prototype.releasePointerCapture) HTMLElement.prototype.releasePointerCapture = function () {};

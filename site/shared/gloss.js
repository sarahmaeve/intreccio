/* Intreccio — click-to-reveal glosses.
 *
 * Click or press Enter/Space on any .gloss element to toggle its
 * revealed state. The blur is visual-only — screen readers always
 * see the plain text, because this is a self-testing aid for sighted
 * readers, not an access-control mechanism.
 *
 * No framework, no dependencies, no build step. */

(function () {
    'use strict';

    function toggle(span) {
        var revealed = span.classList.toggle('revealed');
        span.setAttribute('aria-expanded', revealed ? 'true' : 'false');

        // Swap the action verb in the aria-label so screen-reader
        // users hear the correct next-action. Label format is either
        // "Reveal <LANG> translation" or "Hide <LANG> translation".
        var label = span.getAttribute('aria-label') || '';
        if (revealed) {
            span.setAttribute('aria-label', label.replace(/^Reveal\b/, 'Hide'));
        } else {
            span.setAttribute('aria-label', label.replace(/^Hide\b/, 'Reveal'));
        }
    }

    function onClick(event) {
        var target = event.target.closest('.gloss');
        if (!target) {
            return;
        }
        event.preventDefault();
        toggle(target);
    }

    function onKey(event) {
        if (event.key !== 'Enter' && event.key !== ' ') {
            return;
        }
        var target = event.target;
        if (!target.matches || !target.matches('.gloss')) {
            return;
        }
        event.preventDefault();
        toggle(target);
    }

    function init() {
        document.addEventListener('click', onClick);
        document.addEventListener('keydown', onKey);
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();

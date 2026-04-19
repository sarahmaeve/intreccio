/* Intreccio — drill audio player.
 *
 * Click or press Enter/Space on any [lang="it"][data-audio] span to
 * hear its Italian pronunciation. No page navigation; playback happens
 * in place. Stopping one drill and starting another is seamless.
 *
 * No framework, no dependencies, no build step. */

(function () {
    'use strict';

    var currentAudio = null;
    var currentSpan = null;

    function stopCurrent() {
        if (currentAudio) {
            currentAudio.pause();
            currentAudio.currentTime = 0;
        }
        if (currentSpan) {
            currentSpan.classList.remove('playing');
        }
        currentAudio = null;
        currentSpan = null;
    }

    function play(span) {
        var src = span.dataset.audio;
        if (!src) {
            return;
        }

        // Clicking the currently-playing span stops it.
        if (currentSpan === span && currentAudio && !currentAudio.paused) {
            stopCurrent();
            return;
        }

        stopCurrent();

        // Lazy-create an <audio> element per playback. The DOM can hold
        // hundreds of drill spans per page; keeping a persistent Audio
        // per span would waste memory, and the file is downloaded on
        // first play anyway.
        var audio = new Audio(src);

        audio.addEventListener('ended', function () {
            span.classList.remove('playing');
            if (currentSpan === span) {
                currentAudio = null;
                currentSpan = null;
            }
        });

        audio.addEventListener('error', function () {
            // eslint-disable-next-line no-console
            console.warn('intreccio: drill audio failed to load:', src);
            span.classList.remove('playing');
            if (currentSpan === span) {
                currentAudio = null;
                currentSpan = null;
            }
        });

        currentAudio = audio;
        currentSpan = span;
        span.classList.add('playing');

        var promise = audio.play();
        if (promise && typeof promise.catch === 'function') {
            promise.catch(function (err) {
                // eslint-disable-next-line no-console
                console.warn('intreccio: playback blocked:', err);
                span.classList.remove('playing');
            });
        }
    }

    function onClick(event) {
        var target = event.target.closest('[lang="it"][data-audio]');
        if (!target) {
            return;
        }
        event.preventDefault();
        play(target);
    }

    function onKey(event) {
        if (event.key !== 'Enter' && event.key !== ' ') {
            return;
        }
        var target = event.target;
        if (!target.matches || !target.matches('[lang="it"][data-audio]')) {
            return;
        }
        event.preventDefault();
        play(target);
    }

    function init() {
        // Upgrade every drill span to be keyboard-focusable and
        // screen-reader-announced as a button. We do this in JS rather
        // than emitting these attributes at build time so the static
        // HTML stays minimal and the behaviour degrades gracefully
        // when scripts are disabled (the prose remains readable).
        var spans = document.querySelectorAll('[lang="it"][data-audio]');
        for (var i = 0; i < spans.length; i++) {
            var span = spans[i];
            if (!span.hasAttribute('tabindex')) {
                span.setAttribute('tabindex', '0');
            }
            if (!span.hasAttribute('role')) {
                span.setAttribute('role', 'button');
            }
            if (!span.hasAttribute('aria-label')) {
                span.setAttribute('aria-label', 'Ascolta: ' + span.textContent);
            }
        }

        document.addEventListener('click', onClick);
        document.addEventListener('keydown', onKey);
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();

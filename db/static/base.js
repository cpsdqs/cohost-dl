for (const expandable of document.querySelectorAll('.co-post-contents > .i-expandable')) {
    const cssState = /** @type {HTMLInputElement} */ expandable.querySelector('.i-expanded-state');
    let isOpen = cssState.checked;
    cssState.remove();

    const preview = expandable.querySelector('.i-post-body.i-preview');
    const full = expandable.querySelector('.i-post-body.i-full');

    const render = () => {
        if (isOpen && preview.parentNode) preview.remove();
        if (!isOpen && full.parentNode) full.remove();
        if (isOpen && !full.parentNode) expandable.append(full);
        if (!isOpen && !preview.parentNode) expandable.append(preview);
    };

    let readMore = expandable.querySelector('.i-post-body > .i-read-more');
    let readLess = expandable.querySelector('.i-post-body > .i-read-less');

    for (const readMoreReadLess of [readMore, readLess]) {
        const button = document.createElement('button');
        button.className = readMoreReadLess.className;
        button.textContent = readMoreReadLess.textContent;

        readMoreReadLess.replaceWith(button);

        if (readMoreReadLess === readMore) readMore = button;
        else readLess = button;
    }

    readMore.addEventListener('click', () => {
        isOpen = true;
        render();
    });
    readLess.addEventListener('click', () => {
        isOpen = false;
        const buttonFromTop = readLess.getBoundingClientRect().top;

        render();

        const newButtonPos = window.scrollY + readMore.getBoundingClientRect().top;
        window.scrollTo({ top: newButtonPos - buttonFromTop });
    });

    expandable.classList.remove('has-css-state');
    render();
}

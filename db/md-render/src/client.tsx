import { createRoot, hydrateRoot } from "react-dom/client";
import { PostBodyInner } from "./cohost/preact/components/posts/post-body";
import { chooseAgeRuleset } from "./cohost/lib/markdown/sanitize";
import { WirePostViewModel } from "./cohost/shared/types/wire-models";
import { ReactNode } from "react";
import { Lightbox } from "./cohost/preact/components/lightbox";
import { globalLightboxContext, Lightbox2 } from "./lightbox2";

const lightboxContainer = document.createElement('div');
document.body.append(lightboxContainer);

const lightboxRoot = createRoot(lightboxContainer);
lightboxRoot.render(<Lightbox2 />);

function LightboxProxy({ children }: { children: ReactNode }) {
    return (
        <Lightbox.Provider value={globalLightboxContext}>
            {children}
        </Lightbox.Provider>
    );
}

for (const postContents of document.querySelectorAll('.co-post-contents')) {
    const viewModel: Pick<WirePostViewModel, "blocks" | "astMap" | "postId" | "publishedAt"> = JSON.parse((postContents as HTMLDivElement).dataset.viewModel);
    const ruleset = chooseAgeRuleset(new Date(viewModel.publishedAt));

    for (const postBody of postContents.querySelectorAll('.i-post-body')) {
        const isPreview = postBody.classList.contains('i-preview');
        hydrateRoot(postBody,
            <LightboxProxy>
                <PostBodyInner
                    viewModel={viewModel}
                    renderUntilBlockIndex={
                        isPreview
                            ? (viewModel.astMap.readMoreIndex ?? null)
                            : viewModel.blocks.length
                    }
                    ruleset={ruleset}
                />
            </LightboxProxy>
        );
    }
}

for (const postCollapser of document.querySelectorAll('.co-post-collapser')) {
    const cssState = postCollapser.querySelector('.i-expanded-state') as HTMLInputElement;
    let isOpen = cssState.checked;
    cssState.remove();

    const collapsed = [...postCollapser.children].find((c) => c.classList.contains('i-collapsed'));
    const expanded = [...postCollapser.children].find((c) => c.classList.contains('i-expanded'));

    const render = () => {
        if (isOpen && collapsed.parentNode) collapsed.remove();
        if (!isOpen && expanded.parentNode) expanded.remove();
        if (isOpen && !expanded.parentNode) postCollapser.append(expanded);
        if (!isOpen && !collapsed.parentNode) postCollapser.append(collapsed);
    };

    let expand = collapsed.querySelector('.i-expand-collapsed-button');
    let collapse = expanded.querySelector('.i-collapse-button');

    for (const label of [expand, collapse]) {
        const button = document.createElement('button');
        button.className = label.className;
        button.textContent = label.textContent;

        label.replaceWith(button);

        if (label === expand) expand = button;
        else collapse = button;
    }

    expand.addEventListener('click', () => {
        isOpen = true;
        render();
    });

    collapse.addEventListener('click', () => {
        isOpen = false;
        render();
    });

    postCollapser.classList.remove('has-css-state');
    render();
}

for (const expandable of document.querySelectorAll('.co-post-contents > .i-expandable')) {
    const cssState = expandable.querySelector('.i-expanded-state') as HTMLInputElement;
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

for (const details_ of document.querySelectorAll('.co-themed-titled-box.large\\:expanded')) {
    const details = details_ as HTMLDetailsElement;

    const summary = details.querySelector('summary');

    let expanded = details.open;

    let wasLarge = window.innerWidth >= 1024;
    if (wasLarge) {
        details.open = true;
        details.classList.add('toggle-disabled');
    }

    const update = () => {
        const isLarge = window.innerWidth >= 1024;

        if (!wasLarge && isLarge) {
            details.open = true;
            details.classList.add('toggle-disabled');
        } else if (wasLarge && !isLarge) {
            details.open = expanded;
            details.classList.remove('toggle-disabled');
        }

        wasLarge = isLarge;
    };
    window.addEventListener('resize', update);

    details.addEventListener('toggle', (e) => {
        if (window.innerWidth >= 1024) {
            details.open = true;
            return;
        }

        expanded = details.open;
    });

    summary.addEventListener('click', (e) => {
        e.preventDefault();

        if (window.innerWidth >= 1024) return;

        if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
            details.open = !details.open;
            return;
        }

        const prevHeight = details.offsetHeight;
        details.open = !details.open;
        const newHeight = details.offsetHeight;

        summary.animate([
            { borderBottomLeftRadius: '0', borderBottomRightRadius: '0' },
            { borderBottomLeftRadius: '0', borderBottomRightRadius: '0' },
        ], { duration: 150 });

        details.animate([
            { height: `${prevHeight}px`, overflow: 'clip' },
            { height: `${newHeight}px`, overflow: 'clip' },
        ], {
            duration: 150,
            easing: 'cubic-bezier(0.4, 0, 0.2, 1)',
        });
    });
    summary.classList.add('can-animate');
}

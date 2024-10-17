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

const collapsibleTagsByPost = new Map<string, HTMLDivElement[]>();
for (const collapsibleTags_ of document.querySelectorAll('.collapsible-tags[data-post-id]')) {
    const collapsibleTags = collapsibleTags_ as HTMLDivElement;
    const post = collapsibleTags.dataset.postId;
    if (!collapsibleTagsByPost.has(post)) {
        collapsibleTagsByPost.set(post, [collapsibleTags]);
    } else {
        collapsibleTagsByPost.get(post)!.push(collapsibleTags);
    }
}

for (const collapsibleTags of collapsibleTagsByPost.values()) {
    let expanded = false;
    let shouldCollapse = false;

    const groups = collapsibleTags.map(tags => ({
        tags,
        collapser: tags.querySelector('.i-collapser') as HTMLDivElement,
        items: tags.querySelector('.i-items') as HTMLDivElement,
    }));

    const update = () => {
        for (const { tags, collapser } of groups) {
            if (shouldCollapse && !expanded) {
                tags.classList.add('is-collapsed');
                if (!tags.querySelector('.i-see-all')) {
                    const seeAll = document.createElement('div');
                    seeAll.className = 'i-see-all co-var-post-body-bg';
                    const button = document.createElement('button');
                    button.textContent = 'see all';
                    seeAll.append(button);

                    button.addEventListener('click', () => {
                        expanded = true;
                        update();
                    });

                    collapser.append(seeAll);
                }
            } else {
                tags.classList.remove('is-collapsed');
                tags.querySelector('.i-see-all')?.remove();
            }
        }
    };

    const observer = new ResizeObserver(() => {
        let height: number | null = null;
        let tagHeight: number | null = null;

        // most of them are probably hidden, so just find one that isn't
        for (const { items } of groups) {
            const tag = items.querySelector('.i-tag') as HTMLElement;
            if (!tag) continue;

            if (items.offsetWidth) {
                height = items.offsetHeight;
                tagHeight = tag.offsetHeight;
                break;
            }
        }

        if (height !== null) {
            const prevShouldCollapse = shouldCollapse;
            shouldCollapse = height > tagHeight * 3;
            if (shouldCollapse !== prevShouldCollapse) update();
        }
    });
    groups.forEach(({ items }) => observer.observe(items));
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

for (const nonExpandableBody of document.querySelectorAll('.co-post-contents > .i-post-body')) {
    const body = nonExpandableBody as HTMLDivElement;
    const prose = body.querySelector('.co-prose') as HTMLDivElement | null;

    if (!prose) continue;

    let expanded = false;
    let shouldCollapse = false;

    const forcedReadMore = document.createElement('div');
    forcedReadMore.className = 'i-forced-read-more co-var-post-body-bg';

    const readMore = document.createElement('button');
    readMore.className = 'i-read-more co-read-more-read-less';
    readMore.textContent = 'read more';
    forcedReadMore.append(readMore);

    const readLess = document.createElement('button');
    readLess.textContent = 'read less';
    readLess.className = 'i-read-less co-read-more-read-less';

    const update = () => {
        if (shouldCollapse) {
            if (expanded) {
                body.classList.remove('is-forced-read-more');
                if (forcedReadMore.parentNode) forcedReadMore.remove();
                if (!readLess.parentNode) body.append(readLess);
            } else {
                body.classList.add('is-forced-read-more');
                if (!forcedReadMore.parentNode) body.append(forcedReadMore);
                if (readLess.parentNode) readLess.remove();
            }
        } else {
            body.classList.remove('is-forced-read-more');
            if (forcedReadMore.parentNode) forcedReadMore.remove();
            if (readLess.parentNode) readLess.remove();
        }
    };

    readMore.addEventListener('click', () => {
        expanded = true;
        update();
    });

    readLess.addEventListener('click', () => {
        const buttonFromTop = readLess.getBoundingClientRect().top;

        expanded = false;
        update();

        const newButtonPos = window.scrollY + readMore.getBoundingClientRect().top;
        window.scrollTo({ top: newButtonPos - buttonFromTop });
    });

    const observer = new ResizeObserver(() => {
        const postHeight = prose.offsetHeight;
        const prevShouldCollapse = shouldCollapse;
        shouldCollapse = postHeight > window.innerHeight * 2;
        if (prevShouldCollapse !== shouldCollapse) update();
    });
    observer.observe(prose);
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

    details.addEventListener('toggle', () => {
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

for (const timestamp_ of document.querySelectorAll('time.local-timestamp')) {
    const timestamp = timestamp_ as HTMLTimeElement;

    const date = new Date(timestamp.dateTime);
    const displayFmt = new Intl.DateTimeFormat('en-US', {
        month: 'numeric',
        day: 'numeric',
        year: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
    });
    const titleFmt = new Intl.DateTimeFormat('en-US', {
        weekday: 'short',
        month: 'short',
        day: 'numeric',
        year: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
    });

    const findTextNode = (node: Node) => {
        for (const child of node.childNodes) {
            if (child.nodeType === Node.TEXT_NODE && child.textContent.trim()) {
                return child;
            }
            const found = findTextNode(child);
            if (found) return found;
        }
        return null;
    };
    const textNode = findTextNode(timestamp);
    textNode.textContent = displayFmt.format(date);
    timestamp.title = titleFmt.format(date);
}

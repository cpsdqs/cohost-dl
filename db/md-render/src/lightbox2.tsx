import { useEffect, useRef, useState } from "react";
import { PostId } from "./cohost/shared/types/ids";
import { AttachmentViewBlock } from "./cohost/shared/types/post-blocks";
import { useLightbox } from "./cohost/preact/components/lightbox";
import { flushSync } from "react-dom";

const lightboxContent = new Map<PostId, AttachmentViewBlock[]>();
export const globalLightboxContext: ReturnType<typeof useLightbox> = {
    openLightbox: () => undefined,
    closeLightbox: () => undefined,
    setLightboxContentForPost: (post: PostId, blocks: AttachmentViewBlock[]) => {
        lightboxContent.set(post, blocks);
    },
};

function findPostElementForAttachment(attachment: AttachmentViewBlock) {
    const rPreviewURL = new URL(attachment.attachment.previewURL, location.href).href;
    const rFileURL = new URL(attachment.attachment.fileURL, location.href).href;

    return [...document.querySelectorAll('.i-post-body .group img')].find(img => img.src === rPreviewURL || img.src === rFileURL);
}

export function Lightbox2() {
    const [open, setOpen] = useState(false);
    const [post, setPost] = useState<PostId | null>(null);
    const [attachmentIndex, setAttachmentIndex] = useState(0);

    const dialogRef = useRef<HTMLDialogElement>(null);

    useEffect(() => {
        globalLightboxContext.openLightbox = (postId, attachmentId) => {
            const attachments = lightboxContent.get(postId);
            if (!attachments) return;

            const attachmentIndex = attachments.findIndex(a => a.attachment.attachmentId === attachmentId);
            const attachment = attachments[attachmentIndex];
            if (!attachment) return;

            const openingFromElement = findPostElementForAttachment(attachment);

            if (openingFromElement && document.startViewTransition && !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
                openingFromElement.classList.add('attachment-lightbox-opening-from-this');

                document.startViewTransition(() => {
                    openingFromElement.classList.remove('attachment-lightbox-opening-from-this');

                    flushSync(() => {
                        setPost(postId);
                        setAttachmentIndex(attachmentIndex);
                        setOpen(true);
                    });
                });
            } else {
                setPost(postId);
                setAttachmentIndex(attachmentIndex);
                setOpen(true);
            }
        };
        globalLightboxContext.closeLightbox = () => setOpen(false);
    }, []);

    const isClosing = useRef(false);
    const close = () => {
        if (isClosing.current) return;
        isClosing.current = true;

        const attachment = lightboxContent.get(post)?.[attachmentIndex];
        if (!attachment) {
            setOpen(false);
            return;
        }

        const closingToElement = findPostElementForAttachment(attachment);

        if (closingToElement && document.startViewTransition && !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
            const vt = document.startViewTransition(() => {
                closingToElement.classList.add('attachment-lightbox-opening-from-this');

                flushSync(() => {
                    setOpen(false);
                });
            });

            vt.finished.then(() => {
                closingToElement.classList.remove('attachment-lightbox-opening-from-this');
            });
        } else {
            setOpen(false);
        }
    };

    const onKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'ArrowLeft' || e.key === 'h') {
            setAttachmentIndex(index => Math.max(0, index - 1));
        } else if (e.key === 'ArrowRight' || e.key === 'l') {
            const attachments = lightboxContent.get(post);
            if (!attachments) return;
            setAttachmentIndex(index => Math.min(attachments.length - 1, index + 1));
        }
    };

    useEffect(() => {
        if (open) {
            dialogRef.current.showModal();
            dialogRef.current.scrollTo({ top: 0 });

            window.addEventListener('keydown', onKeyDown);
            return () => window.removeEventListener('keydown', onKeyDown);
        } else {
            dialogRef.current.close();
            isClosing.current = false;
        }
    }, [open]);

    return (
        <dialog
            className="attachment-lightbox"
            ref={dialogRef}
            onCancel={e => {
                e.preventDefault();
                close();
            }}
        >
            <div className="i-backdrop" onClick={close} />

            {open ? (
                <LightboxContents
                    attachments={lightboxContent.get(post)}
                    index={attachmentIndex}
                    onIndexChange={setAttachmentIndex}
                    onClose={close}
                />
            ) : null}
        </dialog>
    );
}

function LightboxContents({ attachments, index, onIndexChange, onClose }: {
    attachments: AttachmentViewBlock[];
    index: number;
    onIndexChange: (index: number) => void;
    onClose: () => void;
}) {
    const current = attachments[index];

    return (
        <div
            className="i-container co-container"
            onClick={e => {
                let cursor = e.target as Node;
                for (let i = 0; i < 10 && cursor; i++) {
                    if (cursor instanceof HTMLButtonElement) return;
                    cursor = cursor.parentNode;
                }
                onClose();
            }}
        >
            <div className="i-top" data-count={attachments.length}>
                <img
                    className="i-attachment"
                    src={current.attachment.fileURL}
                    alt=""
                    onClick={onClose}
                    style={{
                        aspectRatio: `${current.attachment.width} / ${current.attachment.height}`,
                    }}
                />
                {attachments.length > 1 ? (
                    <div className="i-pagination co-pagination-eggs">
                        {index > 0 ? (
                            <button
                                type="button"
                                className="i-button is-prev co-drop-shadow"
                                onClick={() => onIndexChange(index - 1)}
                                aria-label="previous attachment">
                                <PaginationEgg />
                            </button>
                        ) : null}
                        <span className="i-spacer" />
                        {index < attachments.length - 1 ? (
                            <button
                                type="button"
                                className="i-button is-next co-drop-shadow"
                                onClick={() => onIndexChange(index + 1)}
                                aria-label="next attachment">
                                <PaginationEgg />
                            </button>
                        ) : null}
                    </div>
                ) : null}
                {current.attachment.altText ? (
                    <p className="i-alt-text">
                        {current.attachment.altText}
                    </p>
                ) : null}
            </div>
            {attachments.length > 1 ? (
                <ul className="i-bottom" role="group" aria-label="attachments">
                    {attachments.map((attachment, i) => (
                        <li className="i-item" key={attachment.attachment.attachmentId}>
                            <button
                                className="i-button"
                                type="button"
                                data-selected={i === index}
                                onClick={() => onIndexChange(i)}
                            >
                                <img
                                    className="i-preview"
                                    src={attachment.attachment.fileURL}
                                    alt={attachment.attachment.altText}
                                />
                            </button>
                        </li>
                    ))}
                </ul>
            ) : null}
        </div>
    );
}

function PaginationEgg() {
    return (
        <div className="i-egg">
            <svg className="i-arrow" width="24" height="24">
                <path
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="m8.25 4.5 7.5 7.5-7.5 7.5"
                />
            </svg>
        </div>
    );
}

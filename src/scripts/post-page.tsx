import "@/client.tsx";
const rewriteDataNode = document.getElementById('__cohost_dl_rewrite_data');
window.cohostDL = {
    rewriteData: rewriteDataNode ? JSON.parse(rewriteDataNode.innerHTML) : null,
};

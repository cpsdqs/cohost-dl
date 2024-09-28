# cohost-dl 2: DL harder
cohost-dl but downloading a lot more data and with even less usability

Notes:
- You can interrupt this at any time, but if it’s doing something where there’s no progress bar, it’ll start over from page 1.
  This is probably annoying if you were on, like, page 200.
- I am not very good at SQL

Download stages:
1. downloading posts
2. downloading post comments
3. downloading image and audio resources

Files:
- the database: stores all post data
- the output directory: stores all resources like images
- downloader-state.json: file to remember what’s already been downloaded before and skip downloading those things (can be edited)

# Usage in other frameworks

The [`mount_at`](natrix::reactivity::mount::mount_at) function can be used to mount a natrix element at a custom location, this leaks the state so the element stays alive forever.

If the element should be unmounted at some point use [`render_state`](natrix::reactivity::mount::render_state) instead.
This function will return a [`RenderResult`](natrix::reactivity::mount::RenderResult) that should be kept alive until the element is unmounted.
And ideally dropped when the element is unmounted.

> [!IMPORTANT]
> Features that depend on the natrix build pipeline will not work unless the application is built with `natrix build`.
> If you do not wish to build the final application with natrix, you can use the `natrix build` command to build the application and then copy files such as `styles.css` from natrix's `dist` folder to your application.

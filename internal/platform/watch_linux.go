package platform

// RecursiveWatch is false: inotify takes a watch per folder, and the
// user's limit on them is shared with every other app, so only the
// indexed folders themselves are watched.
const RecursiveWatch = false

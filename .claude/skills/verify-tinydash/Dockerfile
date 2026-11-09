# The verification box: a virtual X display, a small window manager, and
# the GTK 3 runtime that mygo loads with dlopen. Without a window manager
# the panel never gets the focus, so hideOnBlur hides it as it shows.
# The app and the automation client come in through a read-only mount,
# built on the Mac; nothing here is TinyDash code.
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      xvfb xauth x11-utils xdotool xclip openbox \
      libgtk-3-0t64 libayatana-appindicator3-1 dbus-x11 \
      ca-certificates fonts-dejavu-core sqlite3 procps \
 && rm -rf /var/lib/apt/lists/*

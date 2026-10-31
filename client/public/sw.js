// §notifications (R8): shows the server's opt-in alerts and brings the game
// forward when one is tapped. Nothing is cached; the game loads as usual.
self.addEventListener("push", (event) => {
  let data = {};
  try { data = event.data ? event.data.json() : {}; } catch { data = { body: event.data ? event.data.text() : "" }; }
  event.waitUntil(self.registration.showNotification(data.title || "Stellar Syndicates", {
    body: data.body || "",
    tag: data.tag || "stellar-syndicates",
    renotify: true,
    icon: "/art/pwa/icon-192.png",
    badge: "/art/pwa/icon-192.png",
  }));
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil((async () => {
    const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    const open = windows.find((client) => "focus" in client);
    return open ? open.focus() : self.clients.openWindow("/");
  })());
});

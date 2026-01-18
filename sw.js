// Service Worker for JX3 Raid Manager PWA
const CACHE_NAME = 'jx3-manager-v1';

self.addEventListener('install', (event) => {
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(clients.claim());
});

self.addEventListener('fetch', (event) => {
  // Basic pass-through strategy for this dynamic environment
  // In a production build, we would cache assets here
  event.respondWith(fetch(event.request));
});
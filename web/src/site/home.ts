// The site's front page. Drapewright installed on a home screen before the
// app moved to /app/ still opens at the root: send it on to the app.

const installed =
  window.matchMedia('(display-mode: standalone)').matches ||
  (navigator as Navigator & { standalone?: boolean }).standalone === true

if (installed) location.replace(`/app/${location.search}${location.hash}`)

const year = document.getElementById('year')
if (year) year.textContent = String(new Date().getFullYear())

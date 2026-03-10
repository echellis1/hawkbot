const apiBaseUrl =
  window.HAWKBOT_CONFIG?.apiBaseUrl ?? 'http://localhost:8080';

document.getElementById('api-base').textContent = apiBaseUrl;
document.getElementById('status').textContent =
  'UI is configured and ready to connect to the Hawkbot API runtime.';

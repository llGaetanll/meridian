// Stripe.js, and the one card form this app has.
//
// Loaded from js.stripe.com on first use rather than bundled: Stripe requires
// it be served from there, and a page that never shows the paywall never
// fetches it. Everything the card form holds lives here, so the wasm side only
// ever passes wells and secrets through.

let loading = null;
let key = null;
let stripe = null;
let parts = [];

// Which mount is the current one. A form taken down while the script is still
// loading must not have its frames put in wells that have left the page.
let generation = 0;

function load() {
  if (window.Stripe) return Promise.resolve();

  if (!loading) {
    loading = new Promise((resolve, reject) => {
      const script = document.createElement("script");
      script.src = "https://js.stripe.com/v3";
      script.onload = () => resolve();
      script.onerror = () => {
        loading = null;
        reject(new Error("js.stripe.com did not load"));
      };
      document.head.appendChild(script);
    });
  }

  return loading;
}

// Take the key a subscription came back with, and start loading Stripe.js
// with it, so the frames are ready by the time the form is drawn.
export function open(publishable) {
  key = publishable;

  load().catch(() => {});
}

// Put Stripe's three frames in the three wells, styled from the wells
// themselves: a frame is its own document and sees none of our stylesheet, so
// it is handed the colours and the face the well has.
export async function mount(number, expiry, cvc) {
  const mine = ++generation;

  if (!key) throw new Error("no publishable key: no subscription has been started");

  await load();

  if (mine !== generation) return;

  destroy();

  stripe ??= window.Stripe(key);

  const look = getComputedStyle(number);
  const token = (name) => look.getPropertyValue(name).trim();

  const style = {
    base: {
      color: look.color,
      fontFamily: look.fontFamily,
      fontSize: look.fontSize,
      fontSmoothing: "antialiased",
      "::placeholder": { color: token("--text-faint") },
    },
    invalid: { color: token("--rose-400") },
  };

  // Set on the well a frame sits in, which is how the field around it lights.
  const classes = { focus: "on", invalid: "bad" };

  const elements = stripe.elements();

  parts = [
    elements.create("cardNumber", { style, classes, showIcon: false }),
    elements.create("cardExpiry", { style, classes }),
    elements.create("cardCvc", { style, classes }),
  ];

  parts[0].mount(number);
  parts[1].mount(expiry);
  parts[2].mount(cvc);
}

export function unmount() {
  generation++;

  destroy();
}

function destroy() {
  for (const part of parts) part.destroy();

  parts = [];
}

export function lock(locked) {
  for (const part of parts) part.update({ disabled: locked });
}

// Pay the invoice the secret is for, with the card in the frames. 3-D Secure,
// where the bank asks for it, is Stripe's own window over the page and is
// over by the time this answers.
//
// Answers with what Stripe said, as JSON, and never throws: a card that was
// refused is the ordinary way this ends, and what it means is the caller's.
export async function confirm(secret) {
  if (!stripe || parts.length === 0) return JSON.stringify({ type: "unmounted" });

  try {
    const { paymentIntent, error } = await stripe.confirmCardPayment(secret, {
      payment_method: { card: parts[0] },
    });

    if (error) {
      return JSON.stringify({
        type: error.type ?? null,
        code: error.code ?? null,
        decline_code: error.decline_code ?? null,
        status: error.payment_intent?.status ?? null,
      });
    }

    return JSON.stringify({ status: paymentIntent?.status ?? null });
  } catch (thrown) {
    return JSON.stringify({ type: "thrown", code: String(thrown?.name ?? thrown) });
  }
}

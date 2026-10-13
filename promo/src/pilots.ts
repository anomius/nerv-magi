/** The demo roster (src/demo.rs): made-up projects and agents, as the footage shows them. */
export const PILOTS = [
  { name: "auth-refactor", kind: "claude", sector: "api-gateway", q: 0, r: -1 },
  { name: "rate-limiter", kind: "claude", sector: "api-gateway", q: 1, r: -1 },
  { name: "checkout-flow", kind: "claude", sector: "storefront", q: -1, r: 0 },
  { name: "e2e-suite", kind: "codex", sector: "storefront", q: 1, r: 0 },
  { name: "push-notify", kind: "claude", sector: "mobile-app", q: -1, r: 1 },
  { name: "tf-plan-review", kind: "codex", sector: "infra", q: 0, r: 1 },
  { name: "k8s-upgrade", kind: "claude", sector: "infra", q: 2, r: -1 },
  { name: "api-reference", kind: "claude", sector: "docs-site", q: -2, r: 1 },
  { name: "release-notes", kind: "codex", sector: "docs-site", q: 1, r: 1 },
  { name: "hypr-rice", kind: "claude", sector: "dotfiles", q: -1, r: -1 },
] as const;

export const SECTORS = 7;

/** Pointy-top axial → pixel. */
export const axial = (q: number, r: number, size: number) => ({
  x: size * Math.sqrt(3) * (q + r / 2),
  y: size * 1.5 * r,
});

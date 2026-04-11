# Mobile UX Audit

Date: 2026-03-12
Updated: 2026-04-11

Scope: iOS and Android front-end screen flows for onboarding, lock, balance, send, receive, seed phrase, and backup.

## Strengths

- The main wallet screen makes `Send` and `Receive` equally prominent, which is the right hierarchy for a wallet. Users do not have to hunt for the two most common tasks.
- Onboarding is generally easy to scan. Both platforms keep the copy tight, the primary CTA obvious, and the create/restore fork visually clear.
- Long-running states are usually acknowledged. Processing, Lightning wait states, and biometric retry now give users visible system feedback instead of silent stalls.
- The receive flow keeps Oubli, Starknet, and Lightning in one place, which is a good mental model when implemented clearly.
- Lightning invoice scanning/pasting now correctly populates the input and shows a review screen before payment on both platforms — the highest-priority trust issue from the original audit is resolved.
- Both platforms use full-screen task flows for Send, Receive, Seed Phrase, and Backup. Android uses a custom `FullScreenTaskDialog`; iOS uses `fullScreenCover`. AlertDialogs are reserved for short confirmations only.
- iOS Receive now uses a labeled segmented control (`Oubli` / `Starknet` / `Lightning`) instead of unlabeled swipe dots, matching Android's `TabRow` pattern.
- iOS primary actions (Review, Send, Pay Invoice, Done) are now in a sticky bottom bar via `safeAreaInset(edge: .bottom)`. The top nav bar only holds Close/Back.
- Android full-screen flows now respect system bars more consistently, and the onboarding / locked / backup paths scroll instead of clipping as readily on smaller layouts or larger text.
- Android Lightning receive no longer traps users inside a non-dismissible waiting state, and expired invoices now fall back to a recoverable create-again path instead of leaving stale invoice UI behind.

## Resolved Issues (since 2026-03-12)

- **Lightning auto-pay on scan/paste**: Fixed. Both platforms now treat scan/paste as input, auto-fill the amount from the BOLT11 invoice, and require explicit review then confirm ("Pay Invoice" button) before sending.
- **Dialog overload (AlertDialog for complex flows)**: Fixed. Android Send, Receive, and Seed Phrase dialogs now use `FullScreenTaskDialog`. iOS Send uses `fullScreenCover`. Alert dialogs are reserved for short confirmations instead of full task flows.
- **iOS Receive swipe-only navigation**: Fixed. Segmented control Picker with `.segmented` style drives a TabView (page dots hidden). Three modes are visible and labeled at a glance.
- **iOS primary actions in top nav bar**: Fixed. All primary actions now live in a bottom action bar. Nav bar is Close/Back only.
- **Backup flow dead-ends**: Improved. iOS now shows a deterministic failed view with error message and "Go Back" button instead of dropping straight to failure. Android backup is a linear local flow (word groups → verification → done) with no spinners or dead-ends.
- **Android seed clipboard handling**: Improved. Onboarding and reveal flows now warn before copying and clear the clipboard after 15 seconds if the copied seed is still present.

## Open UX Issues

- Balance privacy is hidden behind tapping the entire balance card. There is no eye icon, helper text, or explicit affordance on either platform, so users have to guess that the card is interactive.
  Why it is a UX problem: Hidden controls are hard to learn and easy to trigger by accident.
  Likely impact: the feature is easy to miss, and accidental toggles can expose or hide balance unexpectedly in public.
  Recommendation: add a visible eye/eye-slash control in the card header, keep the rest of the card non-tappable, and expose the state clearly to assistive tech.
  Affects: discoverability, accessibility, trust.

- Seed phrase flows still present `Copy to Clipboard` as a normal convenience action during both onboarding and reveal on both platforms. In a privacy wallet, this is a high-risk action being surfaced with almost the same visual weight as the safe path.
  Why it is a UX problem: It normalizes an unsafe behavior in a high-trust moment.
  Likely impact: easier secret leakage, weaker security posture, and mixed trust signals.
  Recommendation: remove clipboard copy for seed phrases, or bury it behind an explicit risk confirmation in an advanced path.
  Affects: trust, error prevention.

- Copy action feedback is still inconsistent. Different flows use different confirmation patterns (`Copied!` label swaps, transient toasts, and snackbars), so the same gesture does not always feel equally confirmed across the app.
  Why it is a UX problem: Inconsistent feedback across the same gesture makes the UI feel unfinished and trains uncertainty.
  Likely impact: repeated taps, missed confirmations, lower trust in the clipboard state.
  Recommendation: add snackbar/toast and light haptic feedback to every copy action across both platforms.
  Affects: usability, trust.

- The iOS onboarding disclaimer is a pseudo-checkbox (Button with SF Symbol `checkmark.square.fill` / `square`) rather than a native Toggle or Checkbox control.
  Why it is a UX problem: Screen readers may not announce it as a toggle, and it does not follow platform interaction conventions.
  Likely impact: accessibility issues for VoiceOver users, slightly unfamiliar feel.
  Recommendation: replace with a native Toggle control or add proper accessibility traits (`accessibilityAddTraits(.isToggle)`).
  Affects: accessibility, platform consistency.

## High-Priority Fixes

- Make the balance visibility control explicit with an eye/eye-slash icon. This is now the most significant daily-use discoverability issue on both platforms.
- Remove or heavily demote seed clipboard copy. Both onboarding and reveal flows on both platforms still surface it prominently.

## Quick Wins

- Unify copy feedback: add snackbar/toast and light haptic to every copy action so copy confirmation feels the same everywhere.
- Replace the iOS disclaimer pseudo-checkbox with a native toggle or add `accessibilityAddTraits(.isToggle)` so VoiceOver announces it correctly.
- Add short helper text under the balance card the first time it appears to hint at tap-to-hide, then remove after first use.

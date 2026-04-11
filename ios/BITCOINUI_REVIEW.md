# BitcoinUI Review — ios/Oubli/Views/

Reviewed: 2026-03-19
Files: 12 SwiftUI files
Result: 0 high, 8 medium across 5 files

---

## BalanceView.swift
Findings: 0 high, 1 medium

### Medium
1. **[A11Y L199-208] QR scanner button frame is 32x32pt, below 44x44pt minimum hit target**
   Fix: Increase the tappable area while keeping the visual size:
   ```swift
   Image(systemName: "camera.fill")
       .font(.caption)
       .foregroundStyle(.white)
       .frame(width: 32, height: 32)
       .background(Color.gray)
       .clipShape(Circle())
       .frame(width: 44, height: 44)  // expand tap target
       .contentShape(Circle())
   ```
   Ref: [iOS HIG — Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility#Touch-targets)

---

## OnboardingView.swift
Findings: 0 high, 2 medium

### Medium
1. **[UX L215-229] "Copy to Clipboard" for seed phrase exposes it to clipboard-reading apps.** While iOS 16+ requires paste permission, other apps on older devices can silently read clipboard contents. No timer clears the clipboard.
   Fix: Either remove the copy button, or auto-clear the clipboard after a short timeout (e.g. 60s) and add a warning label explaining the risk:
   ```swift
   DispatchQueue.main.asyncAfter(deadline: .now() + 60) {
       if UIPasteboard.general.string == mnemonic {
           UIPasteboard.general.string = ""
       }
   }
   ```
   Ref: [Bitcoin Design Guide — Backups](https://bitcoin.design/guide/daily-spending-wallet/backup-and-recovery/landing-page/)

2. **[UX L199-246] Seed phrase display screen has no warning against taking screenshots.** iOS can capture the seed phrase via screenshot or screen recording.
   Fix: Add a visible banner (e.g. "Do not take a screenshot") and optionally detect screenshots via `NotificationCenter` for `.userDidTakeScreenshot` to show an alert.
   Ref: [Bitcoin Design Guide — Backup](https://bitcoin.design/guide/daily-spending-wallet/backup-and-recovery/landing-page/)

---

## SeedPhraseSheet.swift
Findings: 0 high, 2 medium

### Medium
1. **[UX L103-118] Same clipboard risk as OnboardingView — seed phrase copied to clipboard without auto-clear timeout.**
   Fix: Add a 60-second auto-clear of the clipboard after copying, same pattern as OnboardingView fix above.
   Ref: [Bitcoin Design Guide — Backups](https://bitcoin.design/guide/daily-spending-wallet/backup-and-recovery/landing-page/)

2. **[UX L69-121] No screenshot warning when seed phrase is revealed.** Screen recording or screenshots could capture all 12 words.
   Fix: Add a red warning label ("Do not take a screenshot") and listen for `.userDidTakeScreenshot` to show a dismissible alert.
   Ref: [Bitcoin Design Guide — Backup](https://bitcoin.design/guide/daily-spending-wallet/backup-and-recovery/landing-page/)

---

## SendSheet.swift
Findings: 0 high, 1 medium

### Medium
1. **[UX L205-215] Confirmation view shows amount and recipient but no fee breakdown.** For Lightning payments (where fees can vary), and on-chain sends, users should see the fee before confirming.
   Fix: Add a `LabeledContent("Fee", value: ...)` row in the confirmation `Form` section. For Lightning, display the swap/routing fee; for on-chain, show the estimated network fee or note that it is included.
   Ref: [Bitcoin Design Guide — Send fees](https://bitcoin.design/guide/daily-spending-wallet/sending/send-fees/)

---

## ReceiveSheet.swift
Findings: 0 high, 2 medium

### Medium
1. **[QR L213, L251] Lightning invoice QR codes encode the invoice as lowercase.** BOLT11 invoices are case-insensitive, and uppercasing produces significantly more compact QR codes (alphanumeric mode vs. byte mode), improving scannability.
   Fix: Uppercase the invoice string only for QR encoding:
   ```swift
   if let qrImage = generateQRCode(from: invoice.uppercased()) {
   ```
   Display the lowercase version in text as before.
   Ref: [BIP173 — Bech32 encoding for QR](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki)

2. **[A11Y L121-125, L167-171] Address/public-key display uses `.caption2` font which starts very small.** At larger Dynamic Type sizes the zero-width-space wrapping may cause unexpected clipping or truncation.
   Fix: Use `.footnote` or `.caption` instead of `.caption2`, and verify at the largest Accessibility text size that the full address remains readable without clipping.
   Ref: [iOS HIG — Typography](https://developer.apple.com/design/human-interface-guidelines/typography)

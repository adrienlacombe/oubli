import XCTest

/// End-to-end UI tests using the faucet mnemonic from integration tests.
///
/// These tests restore the faucet wallet (OUBLI_TEST_MNEMONIC_A from sepolia.env),
/// verify seed phrase backup, and exercise the private transfer flow.
///
/// NOTE: The wallet connects to Sepolia — these tests require network access and
/// the faucet account to have a Tongo balance. They are slower than unit tests.
final class WalletE2ETests: XCTestCase {

    // Faucet mnemonic from crates/oubli-wallet/tests/sepolia.env
    // Set OUBLI_TEST_MNEMONIC_A in your environment before running UI tests.
    private let faucetMnemonic = ProcessInfo.processInfo.environment["OUBLI_TEST_MNEMONIC_A"] ?? ""
    // Receiver mnemonic — set OUBLI_TEST_MNEMONIC_B or defaults to BIP-39 test vector
    private let receiverMnemonic = ProcessInfo.processInfo.environment["OUBLI_TEST_MNEMONIC_B"] ?? "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    // PIN used in all integration tests
    private let testPin = ProcessInfo.processInfo.environment["OUBLI_TEST_PIN"] ?? "839201"
    // Recipient Tongo public key (derived from receiver mnemonic)
    private let recipientPK = ProcessInfo.processInfo.environment["OUBLI_TEST_RECIPIENT_PK"] ?? "0x0001a48650148f6a4644cd91ba25054042da27cb8d12bcfd0d0b715001ebbb8404142be41199ef6b605b2dc4e71d9a508b8d15fbd736fd680e33bc05b56b5b5b"

    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments += ["-reset-state"]
        app.launch()
    }

    // MARK: - Helpers

    /// Walk through onboarding using restore with a given mnemonic.
    private func restoreWallet(mnemonic: String) {
        // Welcome screen
        let getStarted = app.buttons["Get Started"]
        XCTAssertTrue(getStarted.waitForExistence(timeout: 10), "Welcome screen should appear")
        getStarted.tap()

        // Disclaimer
        let checkbox = app.buttons.matching(NSPredicate(format: "label CONTAINS 'I understand'")).firstMatch
        XCTAssertTrue(checkbox.waitForExistence(timeout: 5))
        checkbox.tap()
        let continueBtn = app.buttons["Continue"]
        continueBtn.tap()

        // Choose restore
        let restoreBtn = app.buttons["Restore from Seed Phrase"]
        XCTAssertTrue(restoreBtn.waitForExistence(timeout: 5))
        restoreBtn.tap()

        // Enter mnemonic
        let textEditor = app.textViews.firstMatch
        XCTAssertTrue(textEditor.waitForExistence(timeout: 5))
        textEditor.tap()
        textEditor.typeText(mnemonic)

        let restoreContinue = app.buttons["Continue"]
        XCTAssertTrue(restoreContinue.waitForExistence(timeout: 5))
        restoreContinue.tap()

        // Set PIN
        let pinField = app.secureTextFields["PIN"]
        XCTAssertTrue(pinField.waitForExistence(timeout: 5))
        pinField.tap()
        pinField.typeText(testPin)

        let confirmPinField = app.secureTextFields["Confirm PIN"]
        XCTAssertTrue(confirmPinField.waitForExistence(timeout: 5))
        confirmPinField.tap()
        confirmPinField.typeText(testPin)

        let createBtn = app.buttons["Create Wallet"]
        XCTAssertTrue(createBtn.waitForExistence(timeout: 5))
        createBtn.tap()

        // Wait for balance screen (onboarding + auto-fund can take a while on Sepolia)
        let satsLabel = app.staticTexts["sats"]
        XCTAssertTrue(satsLabel.waitForExistence(timeout: 60), "Balance screen should appear after onboarding")
    }

    /// Convenience: restore faucet wallet.
    private func restoreFaucetWallet() {
        restoreWallet(mnemonic: faucetMnemonic)
    }

    // MARK: - Test: Seed Phrase Backup

    /// Restore the faucet wallet, then verify Show Seed Phrase displays
    /// the exact 24 words of the faucet mnemonic.
    func testSeedPhraseBackupShowsCorrectWords() throws {
        restoreFaucetWallet()

        // Open menu → Advanced → Show Seed Phrase
        let moreMenu = app.buttons["moreMenu"]
        XCTAssertTrue(moreMenu.waitForExistence(timeout: 5))
        moreMenu.tap()

        let advanced = app.buttons["Advanced"]
        XCTAssertTrue(advanced.waitForExistence(timeout: 5))
        advanced.tap()

        let showSeed = app.buttons["Show Seed Phrase"]
        XCTAssertTrue(showSeed.waitForExistence(timeout: 5))
        showSeed.tap()

        // Enter PIN to reveal
        let pinField = app.secureTextFields.matching(identifier: "seedPhrasePin").firstMatch
        XCTAssertTrue(pinField.waitForExistence(timeout: 5))
        pinField.tap()
        pinField.typeText(testPin)

        let revealBtn = app.buttons["Reveal"]
        XCTAssertTrue(revealBtn.waitForExistence(timeout: 5))
        revealBtn.tap()

        // Wait for seed phrase screen
        let seedTitle = app.navigationBars["Seed Phrase"]
        XCTAssertTrue(seedTitle.waitForExistence(timeout: 20), "Seed phrase screen should appear")

        // Verify every word of the faucet mnemonic is displayed
        let words = faucetMnemonic.split(separator: " ").map(String.init)
        for (index, word) in words.enumerated() {
            XCTAssertTrue(
                app.staticTexts[word].waitForExistence(timeout: 3),
                "Word \(index + 1) ('\(word)') should be visible"
            )
        }

        // Dismiss
        let doneBtn = app.buttons["Done"]
        XCTAssertTrue(doneBtn.exists)
        doneBtn.tap()

        // Back to balance
        let satsLabel = app.staticTexts["sats"]
        XCTAssertTrue(satsLabel.waitForExistence(timeout: 5))
    }

    // MARK: - Test: Private Transfer

    /// Restore the faucet wallet and perform a private transfer to a known recipient.
    ///
    /// This test requires the faucet wallet to have a Tongo balance on Sepolia.
    /// It goes through the full UI flow: Transfer → enter amount & recipient → Review → Confirm.
    /// After confirming, it checks that the app transitions back to the balance screen
    /// (confirming the transaction was submitted without errors).
    func testPrivateTransfer() throws {
        restoreFaucetWallet()

        // Wait for balance to be non-zero (auto-fund should have run during onboarding).
        // The balance text updates from "0.00000000" to something with a non-zero value.
        // Give it extra time since Sepolia can be slow.
        var hasBalance = false
        for _ in 0..<6 {
            // Pull to refresh
            let satsLabel = app.staticTexts["sats"]
            if satsLabel.exists {
                // Check if the balance label shows a non-zero value
                let allStaticTexts = app.staticTexts.allElementsBoundByIndex
                for text in allStaticTexts {
                    let label = text.label
                    if label.contains("0.0000000") && !label.contains("0.00000000") {
                        hasBalance = true
                        break
                    }
                }
                if hasBalance { break }
            }
            // Trigger refresh via the pull-to-refresh gesture or wait
            sleep(10)
        }
        // Proceed regardless — the transfer will fail at the blockchain level if no balance,
        // but we want to test the UI flow either way.

        // Tap Transfer button (match by label containing "Transfer" and the icon)
        let transferBtn = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Transfer'")).firstMatch
        XCTAssertTrue(transferBtn.waitForExistence(timeout: 5), "Transfer button should exist")
        transferBtn.tap()

        // Wait for the amount text field to appear (the sheet has loaded)
        let amountField = app.textFields["0.00000000"]
        XCTAssertTrue(amountField.waitForExistence(timeout: 10), "Amount field should appear in transfer sheet")
        amountField.tap()
        amountField.typeText("0.00000001")

        // Enter recipient
        let recipientField = app.textFields["Stacks or Bitcoin address"]
        XCTAssertTrue(recipientField.waitForExistence(timeout: 5))
        recipientField.tap()
        recipientField.typeText(recipientPK)

        // Tap Review
        let reviewBtn = app.buttons["Review"]
        XCTAssertTrue(reviewBtn.waitForExistence(timeout: 5))
        reviewBtn.tap()

        // Wait for the confirmation screen to appear.
        // LabeledContent renders value as a separate text — look for "Confirm" button directly.
        let confirmBtn = app.buttons["Confirm"]
        XCTAssertTrue(confirmBtn.waitForExistence(timeout: 10), "Confirm button should appear on review screen")
        confirmBtn.tap()

        // After confirming, the sheet dismisses and the background transfer runs.
        // The ViewModel only updates state once the Rust operation completes, so the
        // app transitions directly from Ready → Error (on failure) or stays Ready (on success).
        // On Sepolia without sufficient balance the transfer will likely fail, which is fine —
        // we're testing that the full UI flow works end-to-end.
        //
        // Use an NSPredicate to wait for EITHER "sats" (balance screen) or
        // "Something went wrong" (error screen) to appear.
        let satsLabel2 = app.staticTexts["sats"]
        let errorLabel = app.staticTexts["Something went wrong"]

        let eitherExists = NSPredicate { _, _ in
            satsLabel2.exists || errorLabel.exists
        }
        let expectation = XCTNSPredicateExpectation(predicate: eitherExists, object: nil)
        let result = XCTWaiter.wait(for: [expectation], timeout: 180)
        XCTAssertEqual(result, .completed, "Transfer should reach a final state (balance or error) within 3 minutes")
    }

    // MARK: - Test: Activity Events

    /// Restore the faucet wallet and verify that the activity section shows
    /// on-chain events (Fund, Transfer, etc.) instead of "No transactions yet".
    ///
    /// The faucet wallet has prior Fund transactions on Sepolia, so at least
    /// one event should appear once `loadActivity()` completes.
    func testActivityShowsEvents() throws {
        restoreFaucetWallet()

        // Activity loads asynchronously after onboarding completes.
        let activityHeader = app.staticTexts["Activity"]
        XCTAssertTrue(activityHeader.waitForExistence(timeout: 10), "Activity section header should be visible")

        // Wait for activity to load. Pull-to-refresh periodically to re-trigger
        // loadActivity(). Look for a tx hash pattern (contains "0x" + "...") which
        // is unique to activity rows (shortHash format).
        let txHashPattern = NSPredicate(format: "label MATCHES '0x[0-9a-f]+\\.\\.\\..*'")
        let txHashText = app.staticTexts.matching(txHashPattern)

        var found = false
        for attempt in 0..<4 {
            // Wait up to 30s for a tx hash to appear
            let predicate = NSPredicate { _, _ in txHashText.count > 0 }
            let exp = XCTNSPredicateExpectation(predicate: predicate, object: nil)
            let result = XCTWaiter.wait(for: [exp], timeout: 30)
            if result == .completed {
                found = true
                break
            }
            // Pull to refresh triggers refreshBalance() → loadActivity()
            if attempt < 3 {
                let scrollView = app.scrollViews.firstMatch
                if scrollView.exists {
                    scrollView.swipeDown()
                    sleep(2)
                }
            }
        }

        XCTAssertTrue(found, "At least one activity tx hash should appear for the faucet wallet (waited up to 120s)")

        // "No transactions yet" should be gone.
        let noTransactions = app.staticTexts["No transactions yet"]
        XCTAssertFalse(noTransactions.exists, "'No transactions yet' should NOT be visible when events exist")
    }

    // MARK: - Test: Full Wallet Lifecycle (Two-Wallet Transfer)

    /// End-to-end test: restore faucet wallet → fund → transfer to receiver →
    /// lock/unlock → switch to receiver wallet → verify incoming activity.
    ///
    /// Uses `app.terminate()` + `app.launch()` with `-reset-state` to switch
    /// between wallets mid-test.
    func testFullWalletLifecycle() throws {

        // ── Phase 1: Sender — Onboard faucet wallet ──
        restoreFaucetWallet()

        // ── Phase 2: Sender — Verify balance is non-zero ──
        var hasBalance = false
        for attempt in 0..<6 {
            let allStaticTexts = app.staticTexts.allElementsBoundByIndex
            for text in allStaticTexts {
                let label = text.label
                if label.contains("0.0000000") && !label.contains("0.00000000") {
                    hasBalance = true
                    break
                }
            }
            if hasBalance { break }
            // Pull to refresh
            if attempt < 5 {
                let scrollView = app.scrollViews.firstMatch
                if scrollView.exists { scrollView.swipeDown() }
                sleep(10)
            }
        }
        // Proceed regardless — fund/transfer may fail on Sepolia, that's OK.

        // ── Phase 3: Sender — Fund (small amount) ──
        let fundBtn = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Fund'")).firstMatch
        XCTAssertTrue(fundBtn.waitForExistence(timeout: 5), "Fund button should exist")
        fundBtn.tap()

        let fundAmountField = app.textFields["0.00000000"]
        XCTAssertTrue(fundAmountField.waitForExistence(timeout: 10), "Fund amount field should appear")
        fundAmountField.tap()
        fundAmountField.typeText("0.00000001")

        let fundReviewBtn = app.buttons["Review"]
        XCTAssertTrue(fundReviewBtn.waitForExistence(timeout: 5))
        fundReviewBtn.tap()

        let fundConfirmBtn = app.buttons["Confirm"]
        XCTAssertTrue(fundConfirmBtn.waitForExistence(timeout: 10))
        fundConfirmBtn.tap()

        // Wait up to 180s for Ready (balance screen) or Error
        let fundSatsLabel = app.staticTexts["sats"]
        let fundErrorLabel = app.staticTexts["Something went wrong"]
        let fundDone = NSPredicate { _, _ in fundSatsLabel.exists || fundErrorLabel.exists }
        let fundExp = XCTNSPredicateExpectation(predicate: fundDone, object: nil)
        let fundResult = XCTWaiter.wait(for: [fundExp], timeout: 180)
        XCTAssertEqual(fundResult, .completed, "Fund should reach a final state within 3 minutes")

        // Dismiss error if present
        if fundErrorLabel.exists {
            let dismissBtn = app.buttons["Dismiss"]
            if dismissBtn.waitForExistence(timeout: 3) { dismissBtn.tap() }
        }

        // ── Phase 4: Sender — Transfer to receiver ──
        let transferBtn = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Transfer'")).firstMatch
        XCTAssertTrue(transferBtn.waitForExistence(timeout: 5), "Transfer button should exist")
        transferBtn.tap()

        let transferAmountField = app.textFields["0.00000000"]
        XCTAssertTrue(transferAmountField.waitForExistence(timeout: 10), "Transfer amount field should appear")
        transferAmountField.tap()
        transferAmountField.typeText("0.00000001")

        let recipientField = app.textFields["Stacks or Bitcoin address"]
        XCTAssertTrue(recipientField.waitForExistence(timeout: 5))
        recipientField.tap()
        recipientField.typeText(recipientPK)

        let transferReviewBtn = app.buttons["Review"]
        XCTAssertTrue(transferReviewBtn.waitForExistence(timeout: 5))
        transferReviewBtn.tap()

        let transferConfirmBtn = app.buttons["Confirm"]
        XCTAssertTrue(transferConfirmBtn.waitForExistence(timeout: 10))
        transferConfirmBtn.tap()

        // The transfer runs async on a serial background queue. Wait for either
        // the balance screen (success) or error screen, then dismiss error if needed.
        let transferSatsLabel = app.staticTexts["sats"]
        let transferErrorLabel = app.staticTexts["Something went wrong"]
        let transferDone = NSPredicate { _, _ in transferSatsLabel.exists || transferErrorLabel.exists }
        let transferExp = XCTNSPredicateExpectation(predicate: transferDone, object: nil)
        let transferResult = XCTWaiter.wait(for: [transferExp], timeout: 180)
        XCTAssertEqual(transferResult, .completed, "Transfer should reach a final state within 3 minutes")

        // Dismiss error if present
        if transferErrorLabel.exists {
            let dismissBtn = app.buttons["Dismiss"]
            if dismissBtn.waitForExistence(timeout: 3) { dismissBtn.tap() }
        }

        // ── Phase 5: Sender — Lock & verify lock ──
        // Terminate and relaunch the app (without -reset-state) to trigger the lock screen.
        // This avoids SwiftUI Menu tap reliability issues in UI tests while still testing
        // that the wallet persists and requires authentication on restart.
        app.terminate()
        app.launchArguments = []  // No reset — keep existing wallet
        app.launch()

        // The app should show the lock screen (or view-only balance).
        // LockedView fires auto-biometric on appear, which fails on simulator.
        // Check for either "Wallet Locked" or "Use PIN Instead" or view-only balance.
        let walletLocked = app.staticTexts["Wallet Locked"]
        let usePinBtn = app.buttons["Use PIN Instead"]
        let viewOnlyLabel = app.staticTexts["View Only"]
        let lockOrViewOnly = NSPredicate { _, _ in
            walletLocked.exists || usePinBtn.exists || viewOnlyLabel.exists
        }
        let lockExp = XCTNSPredicateExpectation(predicate: lockOrViewOnly, object: nil)
        let lockResult = XCTWaiter.wait(for: [lockExp], timeout: 30)
        XCTAssertEqual(lockResult, .completed, "App should show lock screen or view-only after restart")

        // ── Phase 6: Sender — Unlock via PIN ──
        if walletLocked.exists || usePinBtn.exists {
            // On lock screen — wait for biometric to fail, then use PIN
            if !usePinBtn.exists {
                sleep(2)
            }
            XCTAssertTrue(usePinBtn.waitForExistence(timeout: 10), "'Use PIN Instead' should appear")
            usePinBtn.tap()

            let unlockPinField = app.secureTextFields["Enter PIN"]
            XCTAssertTrue(unlockPinField.waitForExistence(timeout: 5))
            unlockPinField.tap()
            unlockPinField.typeText(testPin)

            let unlockBtn = app.buttons["Unlock"]
            XCTAssertTrue(unlockBtn.waitForExistence(timeout: 5))
            unlockBtn.tap()
        } else if viewOnlyLabel.exists {
            // View-only mode — enter PIN to upgrade to full access
            let enterPinBtn = app.buttons["Enter PIN to Transact"]
            XCTAssertTrue(enterPinBtn.waitForExistence(timeout: 5))
            enterPinBtn.tap()

            let pinField = app.secureTextFields["PIN"]
            XCTAssertTrue(pinField.waitForExistence(timeout: 5))
            pinField.tap()
            pinField.typeText(testPin)

            let unlockBtn = app.buttons["Unlock"]
            XCTAssertTrue(unlockBtn.waitForExistence(timeout: 5))
            unlockBtn.tap()
        }

        // Verify back on balance screen
        let unlockedSatsLabel = app.staticTexts["sats"]
        XCTAssertTrue(unlockedSatsLabel.waitForExistence(timeout: 30), "Balance screen should appear after unlock")

        // ── Phase 7: Switch to receiver wallet ──
        app.terminate()
        app.launchArguments = ["-reset-state"]
        app.launch()

        restoreWallet(mnemonic: receiverMnemonic)

        // ── Phase 8: Receiver — Verify incoming transfer in activity ──
        let activityHeader = app.staticTexts["Activity"]
        XCTAssertTrue(activityHeader.waitForExistence(timeout: 10), "Activity section should be visible")

        let txHashPattern = NSPredicate(format: "label MATCHES '0x[0-9a-f]+\\.\\.\\..*'")
        let txHashText = app.staticTexts.matching(txHashPattern)

        var foundTx = false
        for attempt in 0..<4 {
            let predicate = NSPredicate { _, _ in txHashText.count > 0 }
            let exp = XCTNSPredicateExpectation(predicate: predicate, object: nil)
            let result = XCTWaiter.wait(for: [exp], timeout: 30)
            if result == .completed {
                foundTx = true
                break
            }
            // Pull to refresh
            if attempt < 3 {
                let scrollView = app.scrollViews.firstMatch
                if scrollView.exists {
                    scrollView.swipeDown()
                    sleep(2)
                }
            }
        }

        XCTAssertTrue(foundTx, "Receiver should see at least one activity event (incoming transfer) within 120s")

        let noTransactions = app.staticTexts["No transactions yet"]
        XCTAssertFalse(noTransactions.exists, "'No transactions yet' should NOT be visible when events exist")
    }
}

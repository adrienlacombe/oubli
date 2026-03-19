import XCTest

/// UI tests for the "Show Seed Phrase" flow.
///
/// Each test performs full onboarding (restore wallet with a known mnemonic,
/// set a PIN) then exercises the seed-phrase reveal feature.
final class SeedPhraseUITests: XCTestCase {

    // Known BIP-39 test mnemonic (12 words)
    private let testMnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    private let testPin = "482957"

    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        // Reset keychain state so we always start from onboarding
        app.launchArguments += ["-reset-state"]
        app.launch()
    }

    // MARK: - Helpers

    /// Walk through the full onboarding flow using restore.
    private func completeOnboarding() {
        // Welcome screen
        let getStarted = app.buttons["Get Started"]
        XCTAssertTrue(getStarted.waitForExistence(timeout: 10), "Welcome screen should appear")
        getStarted.tap()

        // Disclaimer screen — tap the checkbox, then Continue
        let checkbox = app.buttons.matching(NSPredicate(format: "label CONTAINS 'I understand'")).firstMatch
        XCTAssertTrue(checkbox.waitForExistence(timeout: 5), "Disclaimer checkbox should appear")
        checkbox.tap()

        let continueBtn = app.buttons["Continue"]
        XCTAssertTrue(continueBtn.waitForExistence(timeout: 5))
        continueBtn.tap()

        // Create or Restore screen
        let restoreBtn = app.buttons["Restore from Seed Phrase"]
        XCTAssertTrue(restoreBtn.waitForExistence(timeout: 5))
        restoreBtn.tap()

        // Restore mnemonic screen — enter mnemonic in the text editor
        let textEditor = app.textViews.firstMatch
        XCTAssertTrue(textEditor.waitForExistence(timeout: 5), "Mnemonic text editor should appear")
        textEditor.tap()
        textEditor.typeText(testMnemonic)

        let restoreContinue = app.buttons["Continue"]
        XCTAssertTrue(restoreContinue.waitForExistence(timeout: 5))
        restoreContinue.tap()

        // Set PIN screen
        let pinField = app.secureTextFields["PIN"]
        XCTAssertTrue(pinField.waitForExistence(timeout: 5), "PIN field should appear")
        pinField.tap()
        pinField.typeText(testPin)

        let confirmPinField = app.secureTextFields["Confirm PIN"]
        XCTAssertTrue(confirmPinField.waitForExistence(timeout: 5))
        confirmPinField.tap()
        confirmPinField.typeText(testPin)

        let createWalletBtn = app.buttons["Create Wallet"]
        XCTAssertTrue(createWalletBtn.waitForExistence(timeout: 5))
        createWalletBtn.tap()

        // Wait for the balance screen to appear (Ready state).
        // Onboarding involves Argon2id key derivation + RPC calls which can
        // take a while on the simulator, so use a generous timeout.
        let balanceText = app.staticTexts["sats"]
        XCTAssertTrue(balanceText.waitForExistence(timeout: 45), "Balance screen should appear after onboarding")
    }

    /// Open the seed phrase sheet from the overflow menu.
    private func openSeedPhraseSheet() {
        let moreMenu = app.buttons["moreMenu"]
        XCTAssertTrue(moreMenu.waitForExistence(timeout: 5), "More menu button should exist")
        moreMenu.tap()

        // Tap "Advanced" submenu
        let advanced = app.buttons["Advanced"]
        XCTAssertTrue(advanced.waitForExistence(timeout: 5), "Advanced submenu should exist")
        advanced.tap()

        // Tap "Show Seed Phrase"
        let showSeed = app.buttons["Show Seed Phrase"]
        XCTAssertTrue(showSeed.waitForExistence(timeout: 5), "Show Seed Phrase button should exist")
        showSeed.tap()
    }

    // MARK: - Tests

    /// Test that the seed phrase sheet appears and asks for PIN.
    func testShowSeedPhraseSheetAppears() throws {
        completeOnboarding()
        openSeedPhraseSheet()

        // The sheet should show the PIN prompt
        let title = app.staticTexts["Enter PIN to reveal seed phrase"]
        XCTAssertTrue(title.waitForExistence(timeout: 5), "Seed phrase PIN prompt should appear")

        let pinField = app.secureTextFields.matching(identifier: "seedPhrasePin").firstMatch
        XCTAssertTrue(pinField.waitForExistence(timeout: 5), "PIN field should exist in seed phrase sheet")

        // Cancel should dismiss
        let cancelBtn = app.buttons["Cancel"]
        XCTAssertTrue(cancelBtn.exists)
        cancelBtn.tap()

        // Sheet should be dismissed — balance should be visible again
        let satsLabel = app.staticTexts["sats"]
        XCTAssertTrue(satsLabel.waitForExistence(timeout: 5), "Should return to balance screen")
    }

    /// Test that entering the correct PIN reveals the seed words.
    func testCorrectPinRevealsSeedPhrase() throws {
        completeOnboarding()
        openSeedPhraseSheet()

        // Enter PIN
        let pinField = app.secureTextFields.matching(identifier: "seedPhrasePin").firstMatch
        XCTAssertTrue(pinField.waitForExistence(timeout: 5))
        pinField.tap()
        pinField.typeText(testPin)

        // Tap Reveal
        let revealBtn = app.buttons["Reveal"]
        XCTAssertTrue(revealBtn.waitForExistence(timeout: 5))
        revealBtn.tap()

        // Wait for the nav title to change to "Seed Phrase" (indicates success)
        let seedTitle = app.navigationBars["Seed Phrase"]
        XCTAssertTrue(seedTitle.waitForExistence(timeout: 20), "Navigation title should change to 'Seed Phrase'")

        // Verify at least some of the known words are visible
        let words = testMnemonic.split(separator: " ").map(String.init)
        // Check first and last word
        XCTAssertTrue(app.staticTexts[words[0]].waitForExistence(timeout: 5), "First seed word should be visible")
        XCTAssertTrue(app.staticTexts[words[11]].exists, "Last seed word should be visible")

        // Verify the "Done" button exists (confirm we're on the reveal screen)
        let doneBtn = app.buttons["Done"]
        XCTAssertTrue(doneBtn.exists)
        doneBtn.tap()

        // Should return to balance
        let satsLabel = app.staticTexts["sats"]
        XCTAssertTrue(satsLabel.waitForExistence(timeout: 5))
    }

    /// Test that entering a wrong PIN shows an error.
    func testWrongPinShowsError() throws {
        completeOnboarding()
        openSeedPhraseSheet()

        // Enter wrong PIN
        let pinField = app.secureTextFields.matching(identifier: "seedPhrasePin").firstMatch
        XCTAssertTrue(pinField.waitForExistence(timeout: 5))
        pinField.tap()
        pinField.typeText("111111")

        // Tap Reveal
        let revealBtn = app.buttons["Reveal"]
        XCTAssertTrue(revealBtn.waitForExistence(timeout: 5))
        revealBtn.tap()

        // Should show an error, not the seed words
        // Wait a moment for the async call to complete
        sleep(3)

        // The nav title should NOT change to "Seed Phrase"
        let seedTitle = app.navigationBars["Seed Phrase"]
        XCTAssertFalse(seedTitle.exists, "Seed phrase screen should NOT appear with wrong PIN")

        // We should still be on the PIN entry screen
        let pinTitle = app.staticTexts["Enter PIN to reveal seed phrase"]
        XCTAssertTrue(pinTitle.exists, "Should still be on PIN entry screen after wrong PIN")
    }
}

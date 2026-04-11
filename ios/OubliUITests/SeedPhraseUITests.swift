import XCTest

/// Offline UI tests for the current onboarding seed phrase flow.
final class SeedPhraseUITests: XCTestCase {

    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments += ["-reset-state"]
        app.launch()
    }

    private func openGeneratedSeedPhrase() {
        let getStarted = app.buttons["Get Started"]
        XCTAssertTrue(getStarted.waitForExistence(timeout: 10), "Welcome screen should appear")
        getStarted.tap()

        let disclaimerToggle = app.buttons["Disclaimer acknowledgment"]
        XCTAssertTrue(disclaimerToggle.waitForExistence(timeout: 5), "Disclaimer should appear")
        disclaimerToggle.tap()

        let continueButton = app.buttons["Continue"]
        XCTAssertTrue(continueButton.waitForExistence(timeout: 5), "Continue button should appear")
        continueButton.tap()

        let createWallet = app.buttons["Create New Wallet"]
        XCTAssertTrue(createWallet.waitForExistence(timeout: 5), "Create wallet button should appear")
        createWallet.tap()

        let seedTitle = app.staticTexts["Your Seed Phrase"]
        XCTAssertTrue(seedTitle.waitForExistence(timeout: 10), "Seed phrase screen should appear")
    }

    func testSeedPhraseScreenAppearsDuringOnboarding() throws {
        openGeneratedSeedPhrase()

        let writeDownButton = app.buttons["I've Written It Down"]
        XCTAssertTrue(writeDownButton.exists, "Backup confirmation button should appear")

        let warningText = app.staticTexts["Never share your seed phrase. Anyone who has it can steal your funds."]
        XCTAssertTrue(warningText.exists, "Seed phrase warning should be visible")
    }

    func testCopySeedPhraseShowsClipboardWarning() throws {
        openGeneratedSeedPhrase()

        let copyButton = app.buttons["Copy to Clipboard"]
        XCTAssertTrue(copyButton.waitForExistence(timeout: 5), "Copy action should appear")
        copyButton.tap()

        let warningAlert = app.alerts["Clipboard Warning"]
        XCTAssertTrue(warningAlert.waitForExistence(timeout: 5), "Clipboard warning should appear")
        XCTAssertTrue(warningAlert.buttons["Copy Anyway"].exists, "Destructive copy action should appear")
        warningAlert.buttons["Cancel"].tap()

        XCTAssertFalse(warningAlert.exists, "Clipboard warning should dismiss")
    }
}

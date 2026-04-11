import XCTest

/// Networked end-to-end UI tests for the current biometric-only wallet flow.
final class WalletE2ETests: XCTestCase {

    private let faucetMnemonic = ProcessInfo.processInfo.environment["OUBLI_TEST_MNEMONIC_A"] ?? ""
    private let recipientPublicKey = ProcessInfo.processInfo.environment["OUBLI_TEST_RECIPIENT_PK"] ?? "0x0001a48650148f6a4644cd91ba25054042da27cb8d12bcfd0d0b715001ebbb8404142be41199ef6b605b2dc4e71d9a508b8d15fbd736fd680e33bc05b56b5b5b"

    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        guard !faucetMnemonic.isEmpty else {
            throw XCTSkip("OUBLI_TEST_MNEMONIC_A is not set")
        }
        app = XCUIApplication()
        app.launchArguments += ["-reset-state"]
        app.launch()
    }

    private func waitForBalanceScreen(timeout: TimeInterval = 60) {
        let sendButton = app.buttons["sendAction"]
        XCTAssertTrue(
            sendButton.waitForExistence(timeout: timeout),
            "Balance screen should appear after restore"
        )
    }

    private func restoreWallet(mnemonic: String) {
        let restoreButton = app.buttons["I already have a wallet"]
        XCTAssertTrue(restoreButton.waitForExistence(timeout: 10), "Welcome screen should appear")
        restoreButton.tap()

        let textEditor = app.textViews.firstMatch
        XCTAssertTrue(textEditor.waitForExistence(timeout: 5), "Recovery phrase editor should appear")
        textEditor.tap()
        textEditor.typeText(mnemonic)

        let completeRestore = app.buttons["Restore Wallet"]
        XCTAssertTrue(completeRestore.waitForExistence(timeout: 5), "Restore button should appear")
        completeRestore.tap()

        waitForBalanceScreen()
    }

    private func openSeedPhraseSheet() {
        let menuButton = app.buttons["moreMenu"]
        XCTAssertTrue(menuButton.waitForExistence(timeout: 5), "Menu button should exist")
        menuButton.tap()

        let showSeed = app.buttons["Show Seed Phrase"]
        XCTAssertTrue(showSeed.waitForExistence(timeout: 5), "Show Seed Phrase action should exist")
        showSeed.tap()
    }

    func testSeedPhraseBackupShowsCorrectWords() throws {
        restoreWallet(mnemonic: faucetMnemonic)
        openSeedPhraseSheet()

        let revealButton = app.buttons["Reveal"]
        XCTAssertTrue(revealButton.waitForExistence(timeout: 5), "Reveal button should appear")
        revealButton.tap()

        let seedTitle = app.navigationBars["Seed Phrase"]
        XCTAssertTrue(seedTitle.waitForExistence(timeout: 20), "Seed phrase title should appear")

        let words = faucetMnemonic.split(separator: " ").map(String.init)
        for (index, word) in words.enumerated() {
            XCTAssertTrue(
                app.staticTexts[word].waitForExistence(timeout: 3),
                "Word \(index + 1) ('\(word)') should be visible"
            )
        }

        let doneButton = app.buttons["Done"]
        XCTAssertTrue(doneButton.exists)
        doneButton.tap()
    }

    func testPrivateTransferFlowReachesTerminalState() throws {
        restoreWallet(mnemonic: faucetMnemonic)

        let sendButton = app.buttons["Send"]
        XCTAssertTrue(sendButton.waitForExistence(timeout: 5), "Send button should exist")
        sendButton.tap()

        let amountField = app.textFields["Amount in sats"]
        XCTAssertTrue(amountField.waitForExistence(timeout: 10), "Amount field should appear")
        amountField.tap()
        amountField.typeText("100")

        let recipientField = app.textFields["Recipient address or Lightning invoice"]
        XCTAssertTrue(recipientField.waitForExistence(timeout: 5), "Recipient field should appear")
        recipientField.tap()
        recipientField.typeText(recipientPublicKey)

        let sendAction = app.buttons["Send"]
        XCTAssertTrue(sendAction.waitForExistence(timeout: 5), "Primary send button should appear")
        sendAction.tap()

        let confirmAlert = app.alerts["Confirm Send"]
        XCTAssertTrue(confirmAlert.waitForExistence(timeout: 5), "Confirmation alert should appear")
        confirmAlert.buttons["Send"].tap()

        let closeButton = app.buttons["Close"]
        let doneButton = app.buttons["Done"]
        let terminalState = NSPredicate { _, _ in closeButton.exists || doneButton.exists }
        let expectation = XCTNSPredicateExpectation(predicate: terminalState, object: nil)
        let result = XCTWaiter.wait(for: [expectation], timeout: 180)
        XCTAssertEqual(result, .completed, "Send should reach a terminal sheet state")

        if closeButton.exists {
            closeButton.tap()
        } else {
            doneButton.tap()
        }

        waitForBalanceScreen(timeout: 10)
    }

    func testActivityShowsEvents() throws {
        restoreWallet(mnemonic: faucetMnemonic)

        let activityHeader = app.staticTexts["Activity"]
        XCTAssertTrue(activityHeader.waitForExistence(timeout: 10), "Activity header should be visible")

        let firstRow = app.otherElements["activityRow_0"]
        var found = false
        for attempt in 0..<4 {
            if firstRow.waitForExistence(timeout: 30) {
                found = true
                break
            }
            if attempt < 3 {
                let scrollView = app.scrollViews.firstMatch
                if scrollView.exists {
                    scrollView.swipeDown()
                    sleep(2)
                }
            }
        }

        XCTAssertTrue(found, "At least one activity row should appear for the faucet wallet")
        XCTAssertFalse(app.staticTexts["No transactions yet"].exists)
    }
}

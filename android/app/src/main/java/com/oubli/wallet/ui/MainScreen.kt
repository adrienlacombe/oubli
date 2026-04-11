package com.oubli.wallet.ui

import android.app.Activity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.oubli.wallet.ui.balance.BalanceScreen
import com.oubli.wallet.ui.util.shareText
import com.oubli.wallet.viewmodel.ScreenState
import com.oubli.wallet.viewmodel.WalletViewModel

@Composable
fun MainScreen(
    walletViewModel: WalletViewModel = hiltViewModel(),
) {
    val context = LocalContext.current
    val snackbarHostState = remember { SnackbarHostState() }

    LaunchedEffect(Unit) {
        val activity = context as? FragmentActivity
        if (activity != null) {
            walletViewModel.attach(activity)
        }
    }

    val state by walletViewModel.uiState.collectAsStateWithLifecycle()
    val lightningOperation by walletViewModel.lightningOperation.collectAsStateWithLifecycle()

    // Show user messages via snackbar
    val userMessage = state.userMessage
    LaunchedEffect(userMessage) {
        if (userMessage != null) {
            if (userMessage.isError) {
                val result = snackbarHostState.showSnackbar(
                    message = userMessage.text,
                    actionLabel = if (userMessage.diagnostics != null) "Share" else null,
                    duration = SnackbarDuration.Long,
                )
                if (result == SnackbarResult.ActionPerformed && userMessage.diagnostics != null) {
                    shareText(
                        context = context,
                        chooserTitle = "Share diagnostics",
                        subject = "Oubli diagnostics",
                        text = userMessage.diagnostics,
                    )
                }
            } else {
                snackbarHostState.showSnackbar(userMessage.text)
            }
            walletViewModel.onMessageShown(userMessage.id)
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            when (val screen = state.screenState) {
                is ScreenState.Loading -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center,
                    ) {
                        CircularProgressIndicator()
                    }
                }

                is ScreenState.Onboarding -> {
                    OnboardingScreen(
                        onGenerateMnemonic = { callback -> walletViewModel.generateMnemonic(callback) },
                        onValidateMnemonic = { phrase, callback -> walletViewModel.validateMnemonic(phrase, callback) },
                        onSetPin = { pin, callback -> walletViewModel.setPin(pin, callback) },
                        onComplete = { mnemonic -> walletViewModel.completeOnboarding(mnemonic) },
                        onShowMessage = { walletViewModel.showMessage(it) },
                    )
                }

                is ScreenState.Locked -> {
                    LockedScreen(
                        unlockError = screen.unlockError,
                        hasPin = walletViewModel.hasPin(),
                        onUnlockBiometric = { walletViewModel.unlockBiometric() },
                        onUnlockPin = { pin -> walletViewModel.unlockPin(pin) },
                    )
                }

                is ScreenState.Ready -> {
                    BalanceScreen(
                        address = screen.address,
                        publicKey = screen.publicKey,
                        balanceSats = screen.balanceSats,
                        pendingSats = screen.pendingSats,
                        isBalanceHidden = screen.isBalanceHidden,
                        showFiat = screen.showFiat,
                        fiatCurrency = screen.fiatCurrency,
                        onToggleCurrency = { walletViewModel.toggleCurrency() },
                        onSetFiatCurrency = { walletViewModel.setFiatCurrency(it) },
                        satsToFiat = { walletViewModel.satsToFiat(it) },
                        activity = screen.activity,
                        onToggleBalanceHidden = { walletViewModel.toggleBalanceHidden() },
                        onRefresh = { walletViewModel.refreshBalance() },
                        onSend = { amount, recipient -> walletViewModel.send(amount, recipient) },
                        onCalculateSendFee = { amount, recipient ->
                            walletViewModel.calculateSendFee(amount, recipient)
                        },
                        onPayLightning = { bolt11, onResult -> walletViewModel.payLightningWithCallback(bolt11, onResult) },
                        lightningOperation = lightningOperation,
                        onReceiveLightningCreate = { amount, onResult -> walletViewModel.receiveLightningCreateInvoice(amount, onResult) },
                        onReceiveLightningWait = { swapId, onResult -> walletViewModel.receiveLightningWait(swapId, onResult) },
                        onGetMnemonic = { onResult -> walletViewModel.getMnemonic(onResult) },
                        contacts = screen.contacts,
                        onSaveContact = { walletViewModel.saveContact(it) },
                        onDeleteContact = { walletViewModel.deleteContact(it) },
                        satsToFiatRaw = { walletViewModel.satsToFiatRaw(it) },
                        fiatToSats = { walletViewModel.fiatToSats(it) },
                        autoFundIssue = screen.autoFundIssue,
                        isRefreshing = screen.isRefreshing,
                        activityContactNames = screen.activityContactNames,
                        onShowMessage = { walletViewModel.showMessage(it) },
                    )
                }

                is ScreenState.Processing -> {
                    ProcessingScreen(
                        address = screen.address,
                        operation = screen.operation,
                    )
                }

                is ScreenState.Error -> {
                    ErrorScreen(
                        message = screen.message,
                        diagnostics = screen.diagnostics,
                        onRetry = { walletViewModel.refreshBalance() },
                    )
                }

                is ScreenState.SeedBackup -> {
                    SeedBackupScreen(
                        wordGroups = screen.backupState.wordGroups,
                        prompts = screen.backupState.prompts,
                        onVerifyWord = { index, answer, callback ->
                            walletViewModel.verifySeedWord(index, answer, callback)
                        },
                        onDone = { walletViewModel.refreshBalance() },
                    )
                }

                is ScreenState.Wiped -> {
                    WipedScreen(
                        onRestart = {
                            (context as? Activity)?.recreate()
                        },
                    )
                }
            }
        }
    }
}

// ---- Inline utility screens ----

@Composable
private fun ProcessingScreen(address: String, operation: String) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            CircularProgressIndicator()
            Spacer(modifier = Modifier.padding(16.dp))
            Text(text = operation, style = MaterialTheme.typography.titleMedium)
            if (address.isNotEmpty()) {
                Text(
                    text = address,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun ErrorScreen(message: String, diagnostics: String?, onRetry: () -> Unit) {
    val context = LocalContext.current
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(24.dp),
        ) {
            Text(
                text = "Error",
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.error,
            )
            Spacer(modifier = Modifier.padding(8.dp))
            Text(text = message, style = MaterialTheme.typography.bodyMedium)
            Spacer(modifier = Modifier.padding(16.dp))
            Button(onClick = onRetry) { Text("Retry") }
            if (diagnostics != null) {
                Spacer(modifier = Modifier.padding(8.dp))
                Button(
                    onClick = {
                        shareText(
                            context = context,
                            chooserTitle = "Share diagnostics",
                            subject = "Oubli diagnostics",
                            text = diagnostics,
                        )
                    },
                ) {
                    Text("Share Diagnostics")
                }
            }
        }
    }
}

@Composable
private fun WipedScreen(onRestart: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(24.dp),
        ) {
            Text(text = "Wallet Wiped", style = MaterialTheme.typography.headlineSmall)
            Spacer(modifier = Modifier.padding(8.dp))
            Text(
                text = "All data has been erased for your security.",
                style = MaterialTheme.typography.bodyMedium,
            )
            Spacer(modifier = Modifier.padding(16.dp))
            Button(onClick = onRestart) { Text("Set Up New Wallet") }
        }
    }
}

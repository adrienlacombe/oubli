package com.oubli.wallet.ui

import android.app.Activity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.viewmodel.compose.viewModel
import com.oubli.wallet.viewmodel.WalletViewModel
import uniffi.oubli.WalletStateFfi

@Composable
fun MainScreen(
    walletViewModel: WalletViewModel = viewModel(),
) {
    val context = LocalContext.current
    val snackbarHostState = remember { SnackbarHostState() }

    LaunchedEffect(Unit) {
        val activity = context as? FragmentActivity
        if (activity != null) {
            walletViewModel.attach(activity)
        }
    }

    LaunchedEffect(Unit) {
        walletViewModel.errorEvents.collect { message ->
            val result = snackbarHostState.showSnackbar(
                message = message,
                actionLabel = "Copy",
                duration = androidx.compose.material3.SnackbarDuration.Long,
            )
            if (result == androidx.compose.material3.SnackbarResult.ActionPerformed) {
                val clipboard = context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                clipboard.setPrimaryClip(android.content.ClipData.newPlainText("Error", message))
                android.widget.Toast.makeText(context, "Copied to clipboard", android.widget.Toast.LENGTH_SHORT).show()
            }
        }
    }

    LaunchedEffect(Unit) {
        walletViewModel.successEvents.collect { message ->
            snackbarHostState.showSnackbar(message)
        }
    }

    val state by walletViewModel.uiState.collectAsState()
    val seedBackup by walletViewModel.seedBackupState.collectAsState()
    val isBalanceHidden by walletViewModel.isBalanceHidden.collectAsState()
    val showUsd by walletViewModel.showUsd.collectAsState()
    val isRefreshing by walletViewModel.isRefreshing.collectAsState()
    val activity by walletViewModel.activity.collectAsState()
    val lightningOperation by walletViewModel.lightningOperation.collectAsState()
    val biometricUnlockError by walletViewModel.biometricUnlockError.collectAsState()

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            when (state.state) {
                WalletStateFfi.ONBOARDING -> {
                    OnboardingScreen(
                        onGenerateMnemonic = { callback -> walletViewModel.generateMnemonic(callback) },
                        onValidateMnemonic = { phrase, callback -> walletViewModel.validateMnemonic(phrase, callback) },
                        onComplete = { mnemonic -> walletViewModel.completeOnboarding(mnemonic) },
                    )
                }

                WalletStateFfi.LOCKED -> {
                    LockedScreen(
                        unlockError = biometricUnlockError,
                        onUnlockBiometric = { walletViewModel.unlockBiometric() },
                    )
                }

                WalletStateFfi.READY -> {
                    BalanceScreen(
                        address = state.address.orEmpty(),
                        publicKey = state.publicKey.orEmpty(),
                        balanceSats = state.balanceSats ?: "0",
                        pendingSats = state.pendingSats ?: "0",
                        isBalanceHidden = isBalanceHidden,
                        showUsd = showUsd,
                        onToggleCurrency = { walletViewModel.toggleCurrency() },
                        satsToUsd = { walletViewModel.satsToUsd(it) },
                        activity = activity,
                        onToggleBalanceHidden = { walletViewModel.toggleBalanceHidden() },
                        onRefresh = { walletViewModel.refreshBalance() },
                        onSend = { amount, recipient -> walletViewModel.send(amount, recipient) },
                        onPayLightning = { bolt11, onResult -> walletViewModel.payLightningWithCallback(bolt11, onResult) },
                        lightningOperation = lightningOperation,
                        onReceiveLightningCreate = { amount, onResult -> walletViewModel.receiveLightningCreateInvoice(amount, onResult) },
                        onReceiveLightningWait = { swapId, onResult -> walletViewModel.receiveLightningWait(swapId, onResult) },
                        onGetRpcUrl = { walletViewModel.getRpcUrl() },
                        onUpdateRpcUrl = { url -> walletViewModel.updateRpcUrl(url) },
                        onGetMnemonic = { onResult -> walletViewModel.getMnemonic(onResult) },
                        autoFundError = state.autoFundError,
                        isRefreshing = isRefreshing,
                    )
                }

                WalletStateFfi.PROCESSING -> {
                    ProcessingScreen(
                        address = state.address.orEmpty(),
                        operation = state.operation ?: "Processing...",
                    )
                }

                WalletStateFfi.ERROR -> {
                    ErrorScreen(
                        message = state.errorMessage ?: "An unknown error occurred.",
                        onRetry = { walletViewModel.refreshBalance() },
                    )
                }

                WalletStateFfi.SEED_BACKUP -> {
                    val backupState = seedBackup
                    if (backupState != null) {
                        SeedBackupScreen(
                            wordGroups = backupState.wordGroups,
                            prompts = backupState.prompts,
                            onVerifyWord = { index, answer, callback ->
                                walletViewModel.verifySeedWord(index, answer, callback)
                            },
                            onDone = { walletViewModel.refreshBalance() },
                        )
                    } else {
                        Box(
                            modifier = Modifier.fillMaxSize(),
                            contentAlignment = Alignment.Center,
                        ) {
                            CircularProgressIndicator()
                        }
                    }
                }

                WalletStateFfi.WIPED -> {
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
        androidx.compose.foundation.layout.Column(
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            CircularProgressIndicator()
            androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(16.dp))
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
private fun ErrorScreen(message: String, onRetry: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.foundation.layout.Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(24.dp),
        ) {
            Text(
                text = "Error",
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.error,
            )
            androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(8.dp))
            Text(text = message, style = MaterialTheme.typography.bodyMedium)
            androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(16.dp))
            androidx.compose.material3.Button(onClick = onRetry) { Text("Retry") }
        }
    }
}

@Composable
private fun WipedScreen(onRestart: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        androidx.compose.foundation.layout.Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.padding(24.dp),
        ) {
            Text(text = "Wallet Wiped", style = MaterialTheme.typography.headlineSmall)
            androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(8.dp))
            Text(
                text = "All data has been erased for your security.",
                style = MaterialTheme.typography.bodyMedium,
            )
            androidx.compose.foundation.layout.Spacer(modifier = Modifier.padding(16.dp))
            androidx.compose.material3.Button(onClick = onRestart) { Text("Set Up New Wallet") }
        }
    }
}

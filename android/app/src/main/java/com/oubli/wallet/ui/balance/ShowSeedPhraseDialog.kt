package com.oubli.wallet.ui.balance

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.oubli.wallet.ui.components.FullScreenTaskDialog
import com.oubli.wallet.ui.components.TaskPrimaryButton

@Composable
fun ShowSeedPhraseDialog(
    onGetMnemonic: (onResult: (Result<String>) -> Unit) -> Unit,
    onDismiss: () -> Unit,
    onShowMessage: (String) -> Unit = {},
) {
    var seedWords by remember { mutableStateOf<List<String>?>(null) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }
    var isLoading by rememberSaveable { mutableStateOf(false) }
    val context = LocalContext.current

    val bottomBar: (@Composable () -> Unit)? = if (seedWords != null) {
        { TaskPrimaryButton(title = "Done", onClick = onDismiss) }
    } else {
        {
            TaskPrimaryButton(
                title = if (isLoading) "Loading..." else "Reveal",
                enabled = !isLoading,
            ) {
                isLoading = true
                error = null
                onGetMnemonic { result ->
                    isLoading = false
                    result
                        .onSuccess { mnemonic -> seedWords = mnemonic.split(" ") }
                        .onFailure { e -> error = e.message ?: "Failed to retrieve seed phrase" }
                }
            }
        }
    }

    FullScreenTaskDialog(
        title = if (seedWords != null) "Seed Phrase" else "Show Seed Phrase",
        onDismissRequest = onDismiss,
        dismissEnabled = !isLoading,
        bottomBar = bottomBar,
    ) {
        if (seedWords != null) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    text = "Write down these words and store them safely. Anyone with these words can access your funds.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    textAlign = TextAlign.Center,
                )
                Spacer(modifier = Modifier.height(16.dp))
                seedWords!!.forEachIndexed { index, word ->
                    Text(
                        text = "${index + 1}. $word",
                        style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                        modifier = Modifier.padding(vertical = 2.dp),
                    )
                }
                Spacer(modifier = Modifier.height(16.dp))
                Button(onClick = {
                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    clipboard.setPrimaryClip(ClipData.newPlainText("Seed phrase", seedWords!!.joinToString(" ")))
                    onShowMessage("Copied to clipboard")
                }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("Copy to Clipboard")
                }
            }
        } else {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    modifier = Modifier.padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text("Tap Reveal to show your seed phrase.", style = MaterialTheme.typography.bodySmall)
                    if (error != null) {
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(error!!, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
        }
    }
}

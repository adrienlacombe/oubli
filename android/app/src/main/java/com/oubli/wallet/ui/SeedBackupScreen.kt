package com.oubli.wallet.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import uniffi.oubli.VerificationPromptFfi

/**
 * Seed backup screen that displays word groups and then presents verification prompts.
 *
 * Flow:
 *  - Display word groups one at a time (user taps "Next" to advance).
 *  - After viewing all groups, present verification prompts one at a time.
 *  - On success, show a completion confirmation.
 */
@Composable
fun SeedBackupScreen(
    wordGroups: List<List<String>>,
    prompts: List<VerificationPromptFfi>,
    onVerifyWord: (promptIndex: UInt, answer: String, callback: (Boolean) -> Unit) -> Unit,
    onDone: () -> Unit,
) {
    // Phase 0..<wordGroups.size => display groups
    // Phase wordGroups.size..<wordGroups.size+prompts.size => verification
    // Phase wordGroups.size+prompts.size => done
    val totalSteps = wordGroups.size + prompts.size + 1
    var currentStep by rememberSaveable { mutableIntStateOf(0) }
    val verificationResults = remember { mutableStateListOf<Boolean?>() }

    // Initialize verification results tracking.
    if (verificationResults.size < prompts.size) {
        repeat(prompts.size - verificationResults.size) {
            verificationResults.add(null)
        }
    }

    AnimatedContent(
        targetState = currentStep,
        label = "seed_backup_step",
    ) { step ->
        when {
            // Word group display phase
            step < wordGroups.size -> {
                WordGroupDisplay(
                    groupIndex = step,
                    totalGroups = wordGroups.size,
                    words = wordGroups[step],
                    wordOffset = wordGroups.take(step).sumOf { it.size },
                    onNext = { currentStep++ },
                )
            }

            // Verification phase
            step < wordGroups.size + prompts.size -> {
                val promptIdx = step - wordGroups.size
                val prompt = prompts[promptIdx]

                VerificationStep(
                    promptIndex = promptIdx,
                    totalPrompts = prompts.size,
                    wordNumber = prompt.wordNumber.toInt(),
                    previousResult = verificationResults.getOrNull(promptIdx),
                    onSubmit = { answer ->
                        onVerifyWord(promptIdx.toUInt(), answer) { correct ->
                            verificationResults[promptIdx] = correct
                            if (correct) {
                                currentStep++
                            }
                        }
                    },
                )
            }

            // Completion phase
            else -> {
                CompletionScreen(onDone = onDone)
            }
        }
    }
}

@Composable
private fun WordGroupDisplay(
    groupIndex: Int,
    totalGroups: Int,
    words: List<String>,
    wordOffset: Int,
    onNext: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp)
            .verticalScroll(rememberScrollState()),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(modifier = Modifier.height(24.dp))
        Text(
            text = "Backup Your Seed",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Group ${groupIndex + 1} of $totalGroups",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Write down these words carefully. Do not take a screenshot.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(24.dp))

        words.forEachIndexed { index, word ->
            val wordNum = wordOffset + index + 1
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 4.dp),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                ),
            ) {
                Row(
                    modifier = Modifier
                        .padding(horizontal = 16.dp, vertical = 12.dp)
                        .fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "$wordNum.",
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.width(40.dp),
                    )
                    Text(
                        text = word,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Medium,
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(32.dp))
        Button(
            onClick = onNext,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(if (groupIndex < totalGroups - 1) "Next Group" else "Start Verification")
        }
    }
}

@Composable
private fun VerificationStep(
    promptIndex: Int,
    totalPrompts: Int,
    wordNumber: Int,
    previousResult: Boolean?,
    onSubmit: (String) -> Unit,
) {
    var answer by rememberSaveable(promptIndex) { mutableStateOf("") }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            text = "Verify Word",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Verification ${promptIndex + 1} of $totalPrompts",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(24.dp))

        Text(
            text = "What is word #$wordNumber?",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Medium,
        )
        Spacer(modifier = Modifier.height(16.dp))

        OutlinedTextField(
            value = answer,
            onValueChange = { answer = it.lowercase().trim() },
            label = { Text("Word #$wordNumber") },
            modifier = Modifier.fillMaxWidth(0.7f),
            singleLine = true,
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(
                onDone = {
                    if (answer.isNotBlank()) onSubmit(answer)
                },
            ),
            isError = previousResult == false,
            supportingText = if (previousResult == false) {
                { Text("Incorrect. Please try again.") }
            } else {
                null
            },
            trailingIcon = when (previousResult) {
                true -> {
                    {
                        Icon(
                            Icons.Filled.CheckCircle,
                            contentDescription = "Correct",
                            tint = MaterialTheme.colorScheme.primary,
                        )
                    }
                }
                false -> {
                    {
                        Icon(
                            Icons.Filled.Close,
                            contentDescription = "Incorrect",
                            tint = MaterialTheme.colorScheme.error,
                        )
                    }
                }
                null -> null
            },
        )

        Spacer(modifier = Modifier.height(24.dp))
        Button(
            onClick = { onSubmit(answer) },
            modifier = Modifier.fillMaxWidth(0.7f),
            enabled = answer.isNotBlank(),
        ) {
            Text("Verify")
        }
    }
}

@Composable
private fun CompletionScreen(onDone: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(
            Icons.Filled.CheckCircle,
            contentDescription = "Backup complete",
            modifier = Modifier.padding(16.dp),
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "Backup Complete",
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "You have successfully verified your recovery phrase. Keep it safe and never share it.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(modifier = Modifier.height(32.dp))
        Button(
            onClick = onDone,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Done")
        }
    }
}

package com.oubli.wallet.ui.balance

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.oubli.AddressTypeFfi
import uniffi.oubli.ContactAddressFfi
import uniffi.oubli.ContactFfi

private data class EditableAddress(
    val address: String = "",
    val type: AddressTypeFfi = AddressTypeFfi.STARKNET,
    val label: String = "",
)

@Composable
fun ContactDetailDialog(
    contact: ContactFfi?,
    onSave: (ContactFfi) -> Unit,
    onDismiss: () -> Unit,
) {
    val isNew = contact == null
    val name = rememberSaveable { mutableStateOf(contact?.name ?: "") }
    val notes = rememberSaveable { mutableStateOf(contact?.notes ?: "") }
    val addresses = remember {
        mutableStateListOf(
            *(contact?.addresses?.map {
                EditableAddress(it.address, it.addressType, it.label ?: "")
            }?.toTypedArray() ?: arrayOf(EditableAddress()))
        )
    }

    val canSave = name.value.isNotBlank() &&
        addresses.isNotEmpty() &&
        addresses.all { it.address.isNotBlank() }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (isNew) "New Contact" else "Edit Contact") },
        text = {
            Column(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 480.dp)
                        .verticalScroll(rememberScrollState()),
                ) {
                    OutlinedTextField(
                        value = name.value,
                        onValueChange = { name.value = it },
                        label = { Text("Name") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )

                    Spacer(modifier = Modifier.height(12.dp))
                    Text("Addresses", style = MaterialTheme.typography.labelMedium)
                    Spacer(modifier = Modifier.height(4.dp))

                    addresses.forEachIndexed { index, addr ->
                        Column(modifier = Modifier.padding(vertical = 4.dp)) {
                            OutlinedTextField(
                                value = addr.address,
                                onValueChange = { addresses[index] = addr.copy(address = it) },
                                label = { Text("Address") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                            ) {
                                SingleChoiceSegmentedButtonRow(modifier = Modifier.weight(1f)) {
                                    SegmentedButton(
                                        selected = addr.type == AddressTypeFfi.OUBLI,
                                        onClick = { addresses[index] = addr.copy(type = AddressTypeFfi.OUBLI) },
                                        shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
                                    ) { Text("Oubli") }
                                    SegmentedButton(
                                        selected = addr.type == AddressTypeFfi.STARKNET,
                                        onClick = { addresses[index] = addr.copy(type = AddressTypeFfi.STARKNET) },
                                        shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
                                    ) { Text("Starknet") }
                                }
                                if (addresses.size > 1) {
                                    IconButton(onClick = { addresses.removeAt(index) }) {
                                        Icon(Icons.Filled.Delete, "Remove address", tint = MaterialTheme.colorScheme.error)
                                    }
                                }
                            }
                        }
                    }

                    TextButton(onClick = { addresses.add(EditableAddress()) }) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Text("Add Address")
                    }

                    Spacer(modifier = Modifier.height(8.dp))

                    OutlinedTextField(
                        value = notes.value,
                        onValueChange = { notes.value = it },
                        label = { Text("Notes (optional)") },
                        modifier = Modifier.fillMaxWidth(),
                        minLines = 2,
                        maxLines = 4,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    val ffiAddresses = addresses.map {
                        ContactAddressFfi(
                            address = it.address.trim(),
                            addressType = it.type,
                            label = it.label.ifBlank { null },
                        )
                    }
                    onSave(
                        ContactFfi(
                            id = contact?.id ?: "",
                            name = name.value.trim(),
                            addresses = ffiAddresses,
                            notes = notes.value.ifBlank { null },
                            createdAt = contact?.createdAt ?: 0u,
                            lastUsedAt = contact?.lastUsedAt ?: 0u,
                        )
                    )
                },
                enabled = canSave,
            ) { Text("Save") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}

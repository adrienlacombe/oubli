package com.oubli.wallet.ui.balance

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.oubli.wallet.ui.components.FullScreenTaskDialog
import uniffi.oubli.ContactFfi

@Composable
fun ContactListDialog(
    contacts: List<ContactFfi>,
    onSave: (ContactFfi) -> Unit,
    onDelete: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var searchQuery by rememberSaveable { mutableStateOf("") }
    var editingContact by remember { mutableStateOf<ContactFfi?>(null) }
    var showNewContact by remember { mutableStateOf(false) }
    var deleteContactId by remember { mutableStateOf<String?>(null) }

    val filtered = if (searchQuery.isBlank()) contacts else {
        val q = searchQuery.lowercase()
        contacts.filter { c ->
            c.name.lowercase().contains(q) ||
            c.addresses.any { it.address.lowercase().contains(q) }
        }
    }

    FullScreenTaskDialog(
        title = "Contacts",
        onDismissRequest = onDismiss,
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            OutlinedTextField(
                value = searchQuery,
                onValueChange = { searchQuery = it },
                label = { Text("Search") },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                singleLine = true,
            )

            if (filtered.isEmpty()) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 48.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        text = if (searchQuery.isBlank()) "No contacts yet" else "No results",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                ) {
                    items(filtered, key = { it.id }) { contact ->
                        ContactRow(
                            contact = contact,
                            onClick = { editingContact = contact },
                            onDelete = { deleteContactId = contact.id },
                        )
                    }
                }
            }

            // FAB
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                contentAlignment = Alignment.BottomEnd,
            ) {
                FloatingActionButton(onClick = { showNewContact = true }) {
                    Icon(Icons.Filled.Add, contentDescription = "Add contact")
                }
            }
        }
    }

    // Edit dialog
    editingContact?.let { contact ->
        ContactDetailDialog(
            contact = contact,
            onSave = { onSave(it); editingContact = null },
            onDismiss = { editingContact = null },
        )
    }

    // New contact dialog
    if (showNewContact) {
        ContactDetailDialog(
            contact = null,
            onSave = { onSave(it); showNewContact = false },
            onDismiss = { showNewContact = false },
        )
    }

    // Delete confirmation
    deleteContactId?.let { id ->
        val name = contacts.find { it.id == id }?.name ?: ""
        AlertDialog(
            onDismissRequest = { deleteContactId = null },
            title = { Text("Delete Contact?") },
            text = { Text("Remove \"$name\" from your contacts?") },
            confirmButton = {
                TextButton(onClick = { onDelete(id); deleteContactId = null }) {
                    Text("Delete")
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteContactId = null }) {
                    Text("Cancel")
                }
            },
        )
    }
}

@Composable
private fun ContactRow(
    contact: ContactFfi,
    onClick: () -> Unit,
    onDelete: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // Avatar
        Box(
            modifier = Modifier
                .size(40.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.primaryContainer),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = contact.name.take(1).uppercase(),
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onPrimaryContainer,
            )
        }

        Spacer(modifier = Modifier.width(12.dp))

        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = contact.name,
                style = MaterialTheme.typography.bodyLarge,
            )
            contact.addresses.firstOrNull()?.let { addr ->
                Text(
                    text = truncateAddress(addr.address),
                    style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }

        if (contact.addresses.size > 1) {
            Text(
                text = "${contact.addresses.size}",
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier
                    .background(
                        MaterialTheme.colorScheme.surfaceContainerHigh,
                        shape = MaterialTheme.shapes.small,
                    )
                    .padding(horizontal = 6.dp, vertical = 2.dp),
            )
            Spacer(modifier = Modifier.width(8.dp))
        }

        IconButton(onClick = onDelete) {
            Icon(
                Icons.Filled.Delete,
                contentDescription = "Delete",
                tint = MaterialTheme.colorScheme.error,
            )
        }
    }
}

private fun truncateAddress(addr: String): String = com.oubli.wallet.ui.util.truncateAddress(addr)

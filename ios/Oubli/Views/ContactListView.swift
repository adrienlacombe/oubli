import SwiftUI

struct ContactListView: View {
    @ObservedObject var viewModel: WalletViewModel
    @Environment(\.dismiss) private var dismiss
    @State private var searchText: String = ""
    @State private var editingContact: ContactFfi? = nil
    @State private var showNewContact: Bool = false
    @State private var contactToDelete: ContactFfi? = nil

    private var filteredContacts: [ContactFfi] {
        let contacts = viewModel.contacts
        guard !searchText.isEmpty else { return contacts }
        let query = searchText.lowercased()
        return contacts.filter { c in
            c.name.lowercased().contains(query)
            || c.addresses.contains { $0.address.lowercased().contains(query) }
        }
    }

    var body: some View {
        NavigationStack {
            List {
                if filteredContacts.isEmpty {
                    emptyStateView
                        .listRowBackground(Color.clear)
                } else {
                    ForEach(filteredContacts, id: \.id) { contact in
                        contactRow(contact)
                            .contentShape(Rectangle())
                            .onTapGesture { editingContact = contact }
                            .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                                Button(role: .destructive) {
                                    contactToDelete = contact
                                } label: {
                                    Label("Delete", systemImage: "trash")
                                }
                            }
                    }
                }
            }
            .searchable(text: $searchText, prompt: "Search contacts")
            .navigationTitle("Contacts")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        showNewContact = true
                    } label: {
                        Image(systemName: "plus")
                    }
                    .accessibilityLabel("Add contact")
                }
            }
            .sheet(item: $editingContact) { contact in
                ContactDetailSheet(viewModel: viewModel, contact: contact)
            }
            .sheet(isPresented: $showNewContact) {
                ContactDetailSheet(viewModel: viewModel, contact: nil)
            }
            .alert("Delete Contact?", isPresented: .init(
                get: { contactToDelete != nil },
                set: { if !$0 { contactToDelete = nil } }
            )) {
                Button("Delete", role: .destructive) {
                    if let c = contactToDelete {
                        viewModel.deleteContact(id: c.id)
                    }
                    contactToDelete = nil
                }
                Button("Cancel", role: .cancel) {
                    contactToDelete = nil
                }
            } message: {
                if let c = contactToDelete {
                    Text("Remove \"\(c.name)\" from your contacts?")
                }
            }
        }
    }

    private func contactRow(_ contact: ContactFfi) -> some View {
        HStack(spacing: 12) {
            ZStack {
                Circle()
                    .fill(Color.oubliPrimary.opacity(0.15))
                    .frame(width: 40, height: 40)
                Text(String(contact.name.prefix(1)).uppercased())
                    .font(.headline)
                    .foregroundColor(Color.oubliPrimary)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(contact.name)
                    .font(.body.weight(.medium))
                if let first = contact.addresses.first {
                    Text(truncatedAddress(first.address))
                        .font(.caption.monospaced())
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                }
            }
            Spacer()
            if contact.addresses.count > 1 {
                Text("\(contact.addresses.count)")
                    .font(.caption2)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Color.oubliSurfaceContainerHigh)
                    .cornerRadius(8)
            }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(contact.name), \(contact.addresses.count) address\(contact.addresses.count == 1 ? "" : "es")")
    }

    private func truncatedAddress(_ addr: String) -> String {
        guard addr.count > 16 else { return addr }
        return String(addr.prefix(8)) + "..." + String(addr.suffix(6))
    }

    private var emptyStateTitle: String {
        searchText.isEmpty ? "No Contacts" : "No Results"
    }

    private var emptyStateMessage: String {
        searchText.isEmpty
            ? "Tap + to add a contact."
            : "No contacts match \"\(searchText)\"."
    }

    private var emptyStateIcon: String {
        searchText.isEmpty ? "person.crop.circle" : "magnifyingglass"
    }

    @ViewBuilder
    private var emptyStateView: some View {
        if #available(iOS 17.0, *) {
            ContentUnavailableView(
                emptyStateTitle,
                systemImage: emptyStateIcon,
                description: Text(emptyStateMessage)
            )
        } else {
            VStack(spacing: 12) {
                Image(systemName: emptyStateIcon)
                    .font(.system(size: 36))
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                Text(emptyStateTitle)
                    .font(.headline)
                Text(emptyStateMessage)
                    .font(.subheadline)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 32)
        }
    }
}

// Make ContactFfi identifiable for sheet presentation
extension ContactFfi: Identifiable {}

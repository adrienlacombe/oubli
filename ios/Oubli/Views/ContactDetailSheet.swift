import SwiftUI

struct ContactDetailSheet: View {
    @ObservedObject var viewModel: WalletViewModel
    let contact: ContactFfi?
    @Environment(\.dismiss) private var dismiss

    @State private var name: String = ""
    @State private var notes: String = ""
    @State private var addresses: [EditableAddress] = []
    @State private var showDeleteAlert: Bool = false

    private var isNew: Bool { contact == nil }

    private var canSave: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        && !addresses.isEmpty
        && addresses.allSatisfy { !$0.address.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Name") {
                    TextField("Contact name", text: $name)
                        .accessibilityLabel("Contact name")
                }

                Section("Addresses") {
                    ForEach($addresses) { $addr in
                        VStack(alignment: .leading, spacing: 8) {
                            TextField("Address", text: $addr.address)
                                .font(.body.monospaced())
                                .autocorrectionDisabled()
                                .textInputAutocapitalization(.never)
                            Picker("Type", selection: $addr.type) {
                                Text("Oubli (Private)").tag(AddressTypeFfi.oubli)
                                Text("Starknet (L1)").tag(AddressTypeFfi.starknet)
                            }
                            .pickerStyle(.segmented)
                            if !addr.label.isEmpty || addresses.count > 1 {
                                TextField("Label (optional)", text: $addr.label)
                                    .font(.caption)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    .onDelete { offsets in
                        addresses.remove(atOffsets: offsets)
                    }
                    Button {
                        addresses.append(EditableAddress())
                    } label: {
                        Label("Add Address", systemImage: "plus.circle")
                    }
                }

                Section("Notes") {
                    TextField("Optional notes", text: $notes, axis: .vertical)
                        .lineLimit(3...6)
                }

                if !isNew {
                    Section {
                        Button(role: .destructive) {
                            showDeleteAlert = true
                        } label: {
                            Label("Delete Contact", systemImage: "trash")
                        }
                    }
                }
            }
            .navigationTitle(isNew ? "New Contact" : "Edit Contact")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                        .disabled(!canSave)
                }
            }
            .alert("Delete Contact?", isPresented: $showDeleteAlert) {
                Button("Delete", role: .destructive) {
                    if let c = contact {
                        viewModel.deleteContact(id: c.id)
                    }
                    dismiss()
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("This cannot be undone.")
            }
            .onAppear { loadContact() }
        }
    }

    private func loadContact() {
        guard let c = contact else {
            addresses = [EditableAddress()]
            return
        }
        name = c.name
        notes = c.notes ?? ""
        addresses = c.addresses.map { a in
            EditableAddress(
                address: a.address,
                type: a.addressType,
                label: a.label ?? ""
            )
        }
        if addresses.isEmpty {
            addresses = [EditableAddress()]
        }
    }

    private func save() {
        let ffiAddresses = addresses.map { a in
            ContactAddressFfi(
                address: a.address.trimmingCharacters(in: .whitespacesAndNewlines),
                addressType: a.type,
                label: a.label.isEmpty ? nil : a.label
            )
        }
        let ffi = ContactFfi(
            id: contact?.id ?? "",
            name: name.trimmingCharacters(in: .whitespacesAndNewlines),
            addresses: ffiAddresses,
            notes: notes.isEmpty ? nil : notes,
            createdAt: contact?.createdAt ?? 0,
            lastUsedAt: contact?.lastUsedAt ?? 0
        )
        viewModel.saveContact(ffi)
        dismiss()
    }
}

// MARK: - EditableAddress

private struct EditableAddress: Identifiable {
    let id = UUID()
    var address: String = ""
    var type: AddressTypeFfi = .starknet
    var label: String = ""
}

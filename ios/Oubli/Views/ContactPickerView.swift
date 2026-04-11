import SwiftUI

/// Inline contact picker shown in the send flow.
/// Shows recent contacts as tappable chips; selecting one fills the recipient field.
struct ContactPickerView: View {
    @ObservedObject var viewModel: WalletViewModel
    let onSelect: (String) -> Void

    private var recentContacts: [ContactFfi] {
        Array(viewModel.contacts.prefix(5))
    }

    var body: some View {
        if !recentContacts.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("Recent Contacts")
                    .font(.caption)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(recentContacts, id: \.id) { contact in
                            contactChip(contact)
                        }
                    }
                }
            }
        }
    }

    private func contactChip(_ contact: ContactFfi) -> some View {
        Button {
            if let addr = contact.addresses.first {
                onSelect(addr.address)
                viewModel.updateContactLastUsed(id: contact.id)
            }
        } label: {
            HStack(spacing: 6) {
                ZStack {
                    Circle()
                        .fill(Color.oubliPrimary.opacity(0.15))
                        .frame(width: 28, height: 28)
                    Text(String(contact.name.prefix(1)).uppercased())
                        .font(.caption.bold())
                        .foregroundColor(Color.oubliPrimary)
                }
                Text(contact.name)
                    .font(.subheadline)
                    .lineLimit(1)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color.oubliSurfaceContainerHigh)
            .cornerRadius(20)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Send to \(contact.name)")
    }
}

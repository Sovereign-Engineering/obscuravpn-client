import SwiftUI

struct ConditionallyDisabledModifier: ViewModifier {
    let isDisabled: Bool
    let explanation: String
    @State private var showAlert = false

    func body(content: Content) -> some View {
        content
            .disabled(self.isDisabled)
            .opacity(self.isDisabled ? 0.5 : 1.0)
            .onTapGesture {
                if self.isDisabled {
                    self.showAlert = true
                }
            }
            .alert("Not Available", isPresented: self.$showAlert) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(self.explanation)
            }
    }
}

extension View {
    func conditionallyDisabled(
        when isDisabled: Bool,
        explanation: String
    ) -> some View {
        self.modifier(ConditionallyDisabledModifier(
            isDisabled: isDisabled,
            explanation: explanation
        ))
    }
}

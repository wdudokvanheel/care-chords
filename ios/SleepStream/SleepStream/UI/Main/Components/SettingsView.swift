import SwiftUI
import Combine

struct SettingsView: View {
    @Environment(\.presentationMode) var presentationMode
    @StateObject private var serverConfig = ServerConfig.shared
    @State private var serverAddress: String = ""
    @State private var connectionStatus: String = ""
    @State private var isTesting = false
    @State private var cancellable: AnyCancellable?

    var body: some View {
        ZStack {
            // Background
            Color.veryDarkBlue.edgesIgnoringSafeArea(.all)
            
            VStack(spacing: 24) {
                // Header
                Text("Settings")
                    .font(.system(size: 28, weight: .bold))
                    .foregroundColor(.white)
                    .padding(.top, 32)

                // Input Section
                VStack(alignment: .leading, spacing: 8) {
                    Text("Server Address")
                        .foregroundColor(.white.opacity(0.7))
                        .font(.subheadline)
                    
                    TextField("10.0.0.20", text: $serverAddress)
                        .padding()
                        .background(Color.darkerBlue)
                        .cornerRadius(12)
                        .foregroundColor(.white)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(Color.white.opacity(0.1), lineWidth: 1)
                        )
                        .autocapitalization(.none)
                        .disableAutocorrection(true)
                }
                .padding(.horizontal, 24)

                // Actions
                HStack(spacing: 16) {
                    Button(action: testConnection) {
                        HStack {
                            if isTesting {
                                ProgressView()
                                    .progressViewStyle(CircularProgressViewStyle(tint: .white))
                                    .scaleEffect(0.8)
                            }
                            Text(isTesting ? "Testing..." : "Test Connection")
                        }
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(Color.white.opacity(0.1))
                        .foregroundColor(.white)
                        .cornerRadius(12)
                    }
                    .disabled(isTesting)

                    Button(action: saveSettings) {
                        Text("Save")
                            .fontWeight(.semibold)
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(Color.orange)
                            .foregroundColor(.white)
                            .cornerRadius(12)
                    }
                }
                .padding(.horizontal, 24)

                // Status Message
                if !connectionStatus.isEmpty {
                    HStack {
                        Image(systemName: connectionStatus.contains("Success") ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
                        Text(connectionStatus)
                    }
                    .foregroundColor(connectionStatus.contains("Success") ? .green : .red)
                    .font(.subheadline)
                    .padding()
                    .background(Color.black.opacity(0.2))
                    .cornerRadius(8)
                    .transition(.opacity)
                    .animation(.easeInOut, value: connectionStatus)
                }

                Spacer()
                
                // Close Button
                Button(action: {
                    presentationMode.wrappedValue.dismiss()
                }) {
                    Text("Close")
                        .foregroundColor(.white.opacity(0.5))
                        .padding(.bottom, 20)
                }
            }
        }
        .onAppear {
            serverAddress = serverConfig.getURL()
        }
    }

    private func testConnection() {
        isTesting = true
        connectionStatus = "Testing..."
        
        cancellable = serverConfig.validateConnection(url: serverAddress) { success, error in
            isTesting = false
            if success {
                connectionStatus = "Connection Successful!"
            } else {
                connectionStatus = "Connection Failed: \(error ?? "Unknown error")"
            }
        }
    }

    private func saveSettings() {
        serverConfig.saveURL(serverAddress)
        presentationMode.wrappedValue.dismiss()
    }
}

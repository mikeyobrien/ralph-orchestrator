import Foundation
import Security

/// Errors that can occur during Keychain operations.
enum KeychainError: Error, LocalizedError {
    case saveFailed(OSStatus)
    case deleteFailed(OSStatus)
    case dataConversionFailed

    var errorDescription: String? {
        switch self {
        case .saveFailed(let status):
            return "Failed to save to Keychain (status: \(status))"
        case .deleteFailed(let status):
            return "Failed to delete from Keychain (status: \(status))"
        case .dataConversionFailed:
            return "Failed to convert data to/from string"
        }
    }
}

/// Thread-safe Keychain storage manager using Swift actor isolation.
/// Stores sensitive data like API keys securely in the iOS Keychain.
actor KeychainManager {

    /// Keys for stored credentials.
    enum Key: String, CaseIterable {
        case serverAPIKey = "server_api_key"
        case anthropicAPIKey = "anthropic_api_key"
    }

    private let service = "dev.ralph.RalphMobile"

    /// Shared singleton instance for app-wide access.
    static let shared = KeychainManager()

    private init() {}

    /// Save a string value to the Keychain for the specified key.
    /// - Parameters:
    ///   - value: The string value to store.
    ///   - key: The key to associate with the value.
    /// - Throws: `KeychainError.saveFailed` if the operation fails.
    func save(_ value: String, for key: Key) throws {
        guard let data = value.data(using: .utf8) else {
            throw KeychainError.dataConversionFailed
        }

        // Build the query for adding/updating the item
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key.rawValue,
            kSecValueData as String: data
        ]

        // Delete any existing item first (upsert pattern)
        SecItemDelete(query as CFDictionary)

        // Add the new item
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.saveFailed(status)
        }
    }

    /// Retrieve a string value from the Keychain for the specified key.
    /// - Parameter key: The key to look up.
    /// - Returns: The stored string value, or `nil` if not found.
    func get(_ key: Key) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key.rawValue,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess,
              let data = result as? Data,
              let value = String(data: data, encoding: .utf8) else {
            return nil
        }

        return value
    }

    /// Delete a value from the Keychain for the specified key.
    /// - Parameter key: The key to delete.
    /// - Throws: `KeychainError.deleteFailed` if deletion fails (ignores "item not found").
    func delete(_ key: Key) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key.rawValue
        ]

        let status = SecItemDelete(query as CFDictionary)
        // errSecItemNotFound is acceptable - the key just didn't exist
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainError.deleteFailed(status)
        }
    }

    /// Check if a key has a stored value.
    /// - Parameter key: The key to check.
    /// - Returns: `true` if a value exists for this key.
    func exists(_ key: Key) -> Bool {
        return get(key) != nil
    }
}

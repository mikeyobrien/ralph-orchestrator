import Foundation

/// Response from exporting the current configuration.
struct ExportConfigResponse: Decodable {
    let content: String
    let filename: String
}

/// Request body for importing a configuration.
struct ImportConfigRequest: Encodable {
    let content: String
}

/// Response after importing a configuration.
struct ImportConfigResponse: Decodable {
    let status: String
    let path: String
}

// Darkroom helper: PhotoKit and printing for the Rust app.
//
// Usage: darkroom-helper <command> [json]
// Every command reads one JSON object from its second argument (or stdin when the argument is
// "-"), does one thing, prints one JSON object to stdout and exits 0. Errors print
// {"error": "..."} and exit 1. Nothing here is interactive, nothing is cached between runs, and
// nothing is deleted: the write commands only favorite, hide, keyword and file photos into albums.
//
// Commands (read side, milestone 1):
//   status                       -> {authorized, status, libraryPath, assets, albums}
//   albums                       -> {items: [AlbumRow]} the folder and album tree, depth first
//   assets    {album?, limit?, offset?, hidden?}  -> {items: [AssetRow], total}
//   originals {ids: [..]}        -> {items: [{id, path, cached, error?}]}
//   thumbs    {ids: [..], size?, dir}  -> {items: [{id, path, error?}]}
//   keywords  {ids: [..]}        -> {items: [{id, keywords: [..], title?, description?}]}  (via Photos' scripting interface)
// Commands (write side, milestone 3; present now so the Rust side can be tested against them):
//   set       {ids, favorite?, hidden?}            -> {changed}
//   album.create {title, folder?}                  -> {id, cloudId}
//   folder.create {title, parent?}                 -> {id}
//   album.add {album, ids}                         -> {added}
//   keywords.set {items: [{id, add: [..], remove: [..], title?, description?}]} -> {changed, failed: [..]}
//
// Ids are PhotoKit local identifiers. Every row also carries cloudId, Apple's cloud identifier,
// which is the same for the same picture on every Mac signed into the same iCloud account; the
// local id is not. Darkroom stores the cloud id.

import Foundation
import Photos
import AppKit

// MARK: - plumbing

struct HelperError: Error { let message: String }

func fail(_ message: String) -> Never {
    let out = ["error": message]
    if let data = try? JSONSerialization.data(withJSONObject: out) {
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write("\n".data(using: .utf8)!)
    }
    exit(1)
}

func emit(_ object: Any) {
    guard JSONSerialization.isValidJSONObject(object),
          let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]) else {
        fail("internal: result is not JSON")
    }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
}

func readArgs() -> (String, [String: Any]) {
    let argv = CommandLine.arguments
    guard argv.count >= 2 else { fail("usage: darkroom-helper <command> [json]") }
    let cmd = argv[1]
    var params: [String: Any] = [:]
    if argv.count >= 3 {
        let raw = argv[2]
        let data: Data
        if raw == "-" {
            data = FileHandle.standardInput.readDataToEndOfFile()
        } else {
            data = raw.data(using: .utf8) ?? Data()
        }
        if !data.isEmpty {
            guard let obj = try? JSONSerialization.jsonObject(with: data), let dict = obj as? [String: Any] else {
                fail("params must be a JSON object")
            }
            params = dict
        }
    }
    return (cmd, params)
}

let iso: ISO8601DateFormatter = {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return f
}()

func stringList(_ p: [String: Any], _ key: String) -> [String] {
    (p[key] as? [Any])?.compactMap { $0 as? String } ?? []
}

// MARK: - authorization

func ensureAuthorized() -> PHAuthorizationStatus {
    let current = PHPhotoLibrary.authorizationStatus(for: .readWrite)
    if current == .authorized || current == .limited { return current }
    if current == .notDetermined {
        let sem = DispatchSemaphore(value: 0)
        var result: PHAuthorizationStatus = .notDetermined
        PHPhotoLibrary.requestAuthorization(for: .readWrite) { s in
            result = s
            sem.signal()
        }
        // The prompt is answered by a person; give them time.
        _ = sem.wait(timeout: .now() + 600)
        return result
    }
    return current
}

func statusName(_ s: PHAuthorizationStatus) -> String {
    switch s {
    case .authorized: return "authorized"
    case .limited: return "limited"
    case .denied: return "denied"
    case .restricted: return "restricted"
    case .notDetermined: return "notDetermined"
    @unknown default: return "unknown"
    }
}

func requireAccess() {
    let s = ensureAuthorized()
    if s != .authorized && s != .limited {
        fail("Photos access is \(statusName(s)). Allow Darkroom Helper under System Settings > Privacy & Security > Photos.")
    }
}

// MARK: - cloud identifiers

func cloudIds(for localIds: [String]) -> [String: String] {
    var out: [String: String] = [:]
    if localIds.isEmpty { return out }
    let mappings = PHPhotoLibrary.shared().cloudIdentifierMappings(forLocalIdentifiers: localIds)
    for (local, result) in mappings {
        if case .success(let cloud) = result {
            out[local] = cloud.stringValue
        }
    }
    return out
}

func localIds(forCloud cloud: [String]) -> [String: String] {
    var out: [String: String] = [:]
    if cloud.isEmpty { return out }
    let ids = cloud.map { PHCloudIdentifier(stringValue: $0) }
    let mappings = PHPhotoLibrary.shared().localIdentifierMappings(for: ids)
    for (c, result) in mappings {
        if case .success(let local) = result {
            out[c.stringValue] = local
        }
    }
    return out
}

/// Accepts either local identifiers or cloud identifiers (which contain no "/"), and returns assets in the order given.
func fetchAssets(ids: [String]) -> [PHAsset] {
    var locals: [String] = []
    var clouds: [String] = []
    for id in ids {
        if id.contains("/") { locals.append(id) } else { clouds.append(id) }
    }
    let mapped = localIds(forCloud: clouds)
    let wanted = locals + clouds.compactMap { mapped[$0] }
    let fetched = PHAsset.fetchAssets(withLocalIdentifiers: wanted, options: nil)
    var byId: [String: PHAsset] = [:]
    fetched.enumerateObjects { a, _, _ in byId[a.localIdentifier] = a }
    return wanted.compactMap { byId[$0] }
}

// MARK: - rows

func assetRow(_ a: PHAsset, cloud: [String: String], resources: [PHAssetResource]?) -> [String: Any] {
    var row: [String: Any] = [
        "id": a.localIdentifier,
        "cloudId": cloud[a.localIdentifier] ?? "",
        "mediaType": a.mediaType == .image ? "image" : (a.mediaType == .video ? "video" : "other"),
        "width": a.pixelWidth,
        "height": a.pixelHeight,
        "favorite": a.isFavorite,
        "hidden": a.isHidden,
        "burst": a.representsBurst,
        "live": a.mediaSubtypes.contains(.photoLive),
        "raw": false,
        "edited": false,
        "local": true,
    ]
    if let d = a.creationDate { row["created"] = iso.string(from: d) }
    if let d = a.modificationDate { row["modified"] = iso.string(from: d) }
    if let loc = a.location {
        row["lat"] = loc.coordinate.latitude
        row["lon"] = loc.coordinate.longitude
    }
    if let id = a.burstIdentifier { row["burstId"] = id }
    if let res = resources {
        var names: [String] = []
        var raw = false
        var edited = false
        let local = true
        for r in res {
            switch r.type {
            case .photo, .video:
                names.append(r.originalFilename)
                if r.uniformTypeIdentifier.contains("raw-image") || r.uniformTypeIdentifier.hasPrefix("com.adobe.raw") { raw = true }
            case .alternatePhoto:
                raw = true
                names.append(r.originalFilename)
            case .fullSizePhoto, .fullSizeVideo, .adjustmentData:
                edited = true
            default: break
            }
        }
        row["filename"] = names.first ?? ""
        row["filenames"] = names
        row["raw"] = raw
        row["edited"] = edited
        row["local"] = local
    }
    return row
}

func albumRow(_ c: PHCollection, depth: Int, parent: String?, cloud: [String: String]) -> [String: Any] {
    var row: [String: Any] = [
        "id": c.localIdentifier,
        "cloudId": cloud[c.localIdentifier] ?? "",
        "title": c.localizedTitle ?? "",
        "depth": depth,
        "parent": parent ?? "",
    ]
    if let list = c as? PHCollectionList {
        row["kind"] = "folder"
        row["count"] = PHCollection.fetchCollections(in: list, options: nil).count
    } else if let album = c as? PHAssetCollection {
        switch album.assetCollectionType {
        case .album: row["kind"] = "album"
        case .smartAlbum: row["kind"] = "smart"
        default: row["kind"] = "other"
        }
        row["subtype"] = album.assetCollectionSubtype.rawValue
        row["count"] = album.estimatedAssetCount == NSNotFound ? -1 : album.estimatedAssetCount
    }
    return row
}

// MARK: - commands

func cmdStatus() {
    let s = ensureAuthorized()
    var out: [String: Any] = ["status": statusName(s), "authorized": s == .authorized || s == .limited]
    if s == .authorized || s == .limited {
        let opts = PHFetchOptions()
        out["assets"] = PHAsset.fetchAssets(with: opts).count
        out["albums"] = PHAssetCollection.fetchAssetCollections(with: .album, subtype: .any, options: nil).count
        // The system library path, for information only. Darkroom never writes inside it.
        if let url = PHPhotoLibrary.shared().value(forKey: "photoLibraryURL") as? URL {
            out["libraryPath"] = url.path
        }
    }
    emit(out)
}

func walk(_ collections: PHFetchResult<PHCollection>, depth: Int, parent: String?, into rows: inout [[String: Any]], ids: inout [String]) {
    for i in 0..<collections.count {
        let c = collections.object(at: i)
        ids.append(c.localIdentifier)
        rows.append(["_": c, "depth": depth, "parent": parent ?? ""])
        if let list = c as? PHCollectionList {
            let children = PHCollection.fetchCollections(in: list, options: nil)
            walk(children, depth: depth + 1, parent: c.localIdentifier, into: &rows, ids: &ids)
        }
    }
}

func cmdAlbums() {
    requireAccess()
    var raw: [[String: Any]] = []
    var ids: [String] = []
    let top = PHCollectionList.fetchTopLevelUserCollections(with: nil)
    walk(top, depth: 0, parent: nil, into: &raw, ids: &ids)
    // Smart albums Photos keeps for everyone (Favorites, Hidden, Recently Deleted, ...)
    let smart = PHAssetCollection.fetchAssetCollections(with: .smartAlbum, subtype: .any, options: nil)
    let wanted: Set<PHAssetCollectionSubtype> = [.smartAlbumFavorites, .smartAlbumAllHidden, .smartAlbumRecentlyAdded, .smartAlbumUserLibrary, .smartAlbumBursts, .smartAlbumRAW, .smartAlbumLivePhotos, .smartAlbumPanoramas, .smartAlbumScreenshots, .smartAlbumVideos]
    smart.enumerateObjects { c, _, _ in
        if wanted.contains(c.assetCollectionSubtype) {
            ids.append(c.localIdentifier)
            raw.append(["_": c, "depth": 0, "parent": "smart"])
        }
    }
    let cloud = cloudIds(for: ids)
    let rows = raw.map { r -> [String: Any] in
        albumRow(r["_"] as! PHCollection, depth: r["depth"] as! Int, parent: (r["parent"] as! String).isEmpty ? nil : (r["parent"] as! String), cloud: cloud)
    }
    emit(["items": rows])
}

func cmdAssets(_ p: [String: Any]) {
    requireAccess()
    let opts = PHFetchOptions()
    opts.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: true)]
    if p["hidden"] as? Bool == true {
        opts.includeHiddenAssets = true
    }
    var result: PHFetchResult<PHAsset>
    if let albumId = p["album"] as? String, !albumId.isEmpty {
        let cols = PHAssetCollection.fetchAssetCollections(withLocalIdentifiers: [albumId], options: nil)
        guard let col = cols.firstObject else { fail("no album with id \(albumId)") }
        result = PHAsset.fetchAssets(in: col, options: opts)
    } else {
        result = PHAsset.fetchAssets(with: opts)
    }
    let total = result.count
    let offset = max(0, p["offset"] as? Int ?? 0)
    let limit = max(1, min(p["limit"] as? Int ?? 5000, 20000))
    let end = min(total, offset + limit)
    var assets: [PHAsset] = []
    if offset < end {
        result.enumerateObjects(at: IndexSet(integersIn: offset..<end), options: []) { a, _, _ in assets.append(a) }
    }
    let cloud = cloudIds(for: assets.map { $0.localIdentifier })
    let withResources = p["resources"] as? Bool ?? true
    let rows = assets.map { a in
        assetRow(a, cloud: cloud, resources: withResources ? PHAssetResource.assetResources(for: a) : nil)
    }
    emit(["items": rows, "total": total, "offset": offset])
}

func cacheDir(_ sub: String) -> URL {
    let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        .appendingPathComponent("Darkroom", isDirectory: true)
        .appendingPathComponent(sub, isDirectory: true)
    try? FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
    return base
}

func safeName(_ s: String) -> String {
    let bad = CharacterSet(charactersIn: "/\\:")
    return s.components(separatedBy: bad).joined(separator: "_")
}

/// The original file for an asset. For a RAW+JPEG pair the RAW is the original. Local originals
/// are written once into Darkroom's cache (PhotoKit does not hand out paths inside the library
/// package, and Darkroom never reaches into it); iCloud-only originals are downloaded first.
func cmdOriginals(_ p: [String: Any]) {
    requireAccess()
    let ids = stringList(p, "ids")
    if ids.isEmpty { fail("ids required") }
    let dir = cacheDir("originals")
    let assets = fetchAssets(ids: ids)
    let cloud = cloudIds(for: assets.map { $0.localIdentifier })
    var items: [[String: Any]] = []
    let group = DispatchGroup()
    let lock = NSLock()
    for a in assets {
        let resources = PHAssetResource.assetResources(for: a)
        let original = resources.first { $0.type == .alternatePhoto }
            ?? resources.first { $0.type == .photo || $0.type == .video }
        guard let res = original else {
            items.append(["id": a.localIdentifier, "cloudId": cloud[a.localIdentifier] ?? "", "error": "no original resource"])
            continue
        }
        let key = (cloud[a.localIdentifier] ?? a.localIdentifier)
        let ext = (res.originalFilename as NSString).pathExtension
        let name = safeName(key) + (ext.isEmpty ? "" : "." + ext)
        let dest = dir.appendingPathComponent(name)
        if FileManager.default.fileExists(atPath: dest.path) {
            items.append(["id": a.localIdentifier, "cloudId": key, "path": dest.path, "cached": true, "filename": res.originalFilename])
            continue
        }
        let ro = PHAssetResourceRequestOptions()
        ro.isNetworkAccessAllowed = true
        group.enter()
        PHAssetResourceManager.default().writeData(for: res, toFile: dest, options: ro) { err in
            lock.lock()
            if let err = err {
                try? FileManager.default.removeItem(at: dest)
                items.append(["id": a.localIdentifier, "cloudId": key, "error": err.localizedDescription])
            } else {
                items.append(["id": a.localIdentifier, "cloudId": key, "path": dest.path, "cached": false, "filename": res.originalFilename])
            }
            lock.unlock()
            group.leave()
        }
    }
    _ = group.wait(timeout: .now() + 3600)
    emit(["items": items])
}

/// Thumbnails as JPEG files, sized to fit `size` pixels on the long edge (default 512), from
/// Photos' own rendered previews, so they show the edit Photos shows.
func cmdThumbs(_ p: [String: Any]) {
    requireAccess()
    let ids = stringList(p, "ids")
    if ids.isEmpty { fail("ids required") }
    let size = CGFloat(p["size"] as? Int ?? 512)
    let dir: URL = (p["dir"] as? String).map { URL(fileURLWithPath: $0, isDirectory: true) } ?? cacheDir("thumbs")
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let assets = fetchAssets(ids: ids)
    let cloud = cloudIds(for: assets.map { $0.localIdentifier })
    let opts = PHImageRequestOptions()
    opts.isSynchronous = true
    opts.deliveryMode = .highQualityFormat
    opts.resizeMode = .exact
    opts.isNetworkAccessAllowed = true
    var items: [[String: Any]] = []
    for a in assets {
        let key = cloud[a.localIdentifier] ?? a.localIdentifier
        let dest = dir.appendingPathComponent(safeName(key) + "_\(Int(size)).jpg")
        if FileManager.default.fileExists(atPath: dest.path) {
            items.append(["id": a.localIdentifier, "cloudId": key, "path": dest.path])
            continue
        }
        var written = false
        var message = "no image"
        PHImageManager.default().requestImage(for: a, targetSize: CGSize(width: size, height: size), contentMode: .aspectFit, options: opts) { img, info in
            if let img = img, let tiff = img.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff),
               let jpeg = rep.representation(using: .jpeg, properties: [.compressionFactor: 0.85]) {
                do {
                    try jpeg.write(to: dest)
                    written = true
                } catch {
                    message = error.localizedDescription
                }
            } else if let err = info?[PHImageErrorKey] as? NSError {
                message = err.localizedDescription
            }
        }
        if written {
            items.append(["id": a.localIdentifier, "cloudId": key, "path": dest.path])
        } else {
            items.append(["id": a.localIdentifier, "cloudId": key, "error": message])
        }
    }
    emit(["items": items])
}

// MARK: - Photos scripting (keywords, titles, captions)

/// Runs Photos' scripting interface for what PhotoKit cannot do. The script is constant; every
/// value reaches it as an argument, so nothing needs escaping and no id can change what it does.
func runAppleScript(_ source: String, args: [String]) throws -> String {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
    task.arguments = ["-e", source, "--"] + args
    let out = Pipe()
    let err = Pipe()
    task.standardOutput = out
    task.standardError = err
    try task.run()
    let data = out.fileHandleForReading.readDataToEndOfFile()
    let errData = err.fileHandleForReading.readDataToEndOfFile()
    task.waitUntilExit()
    if task.terminationStatus != 0 {
        throw HelperError(message: String(data: errData, encoding: .utf8) ?? "osascript failed")
    }
    return String(data: data, encoding: .utf8) ?? ""
}

/// Photos' scripting ids are the PhotoKit local identifier's UUID followed by "/L0/001".
func scriptId(_ localId: String) -> String {
    let uuid = localId.split(separator: "/").first.map(String.init) ?? localId
    return uuid + "/L0/001"
}

let keywordsReadScript = """
on run argv
    set out to ""
    tell application "Photos"
        repeat with u in argv
            try
                set m to media item id (contents of u)
                set kw to keywords of m
                if kw is missing value then set kw to {}
                set t to name of m
                if t is missing value then set t to ""
                set d to description of m
                if d is missing value then set d to ""
                set AppleScript's text item delimiters to (ASCII character 31)
                set out to out & (contents of u) & (ASCII character 30) & (kw as text) & (ASCII character 30) & t & (ASCII character 30) & d & (ASCII character 29)
                set AppleScript's text item delimiters to ""
            end try
        end repeat
    end tell
    return out
end run
"""

func cmdKeywords(_ p: [String: Any]) {
    requireAccess()
    let ids = stringList(p, "ids")
    if ids.isEmpty { fail("ids required") }
    let assets = fetchAssets(ids: ids)
    let cloud = cloudIds(for: assets.map { $0.localIdentifier })
    var items: [[String: Any]] = []
    // Photos handles a few hundred ids per call comfortably.
    for chunk in stride(from: 0, to: assets.count, by: 200) {
        let slice = Array(assets[chunk..<min(assets.count, chunk + 200)])
        let args = slice.map { scriptId($0.localIdentifier) }
        guard let text = try? runAppleScript(keywordsReadScript, args: args) else { continue }
        let byScriptId = Dictionary(uniqueKeysWithValues: slice.map { (scriptId($0.localIdentifier), $0) })
        for rec in text.components(separatedBy: "\u{1D}") where !rec.isEmpty {
            let f = rec.components(separatedBy: "\u{1E}")
            guard f.count >= 4, let a = byScriptId[f[0]] else { continue }
            let kws = f[1].isEmpty ? [] : f[1].components(separatedBy: "\u{1F}")
            items.append(["id": a.localIdentifier, "cloudId": cloud[a.localIdentifier] ?? "", "keywords": kws, "title": f[2], "description": f[3]])
        }
    }
    emit(["items": items])
}

let keywordsWriteScript = """
on run argv
    -- argv: id, title-or-\\u0001, description-or-\\u0001, then "+kw" / "-kw" entries
    set u to item 1 of argv
    set newTitle to item 2 of argv
    set newDesc to item 3 of argv
    tell application "Photos"
        set m to media item id u
        if newTitle is not (ASCII character 1) then set name of m to newTitle
        if newDesc is not (ASCII character 1) then set description of m to newDesc
        set kw to keywords of m
        if kw is missing value then set kw to {}
        if (count of argv) > 3 then
            repeat with i from 4 to (count of argv)
                set e to item i of argv
                set op to text 1 of e
                set k to text 2 thru -1 of e
                if op is "+" then
                    if kw does not contain k then set kw to kw & {k}
                else if op is "-" then
                    set kept to {}
                    repeat with x in kw
                        if (contents of x) is not k then set end of kept to contents of x
                    end repeat
                    set kw to kept
                end if
            end repeat
            set keywords of m to kw
        end if
    end tell
end run
"""

func cmdKeywordsSet(_ p: [String: Any]) {
    requireAccess()
    guard let items = p["items"] as? [[String: Any]], !items.isEmpty else { fail("items required") }
    var changed = 0
    var failed: [[String: Any]] = []
    for it in items {
        guard let id = it["id"] as? String else { continue }
        guard let asset = fetchAssets(ids: [id]).first else {
            failed.append(["id": id, "error": "not found"])
            continue
        }
        var args = [scriptId(asset.localIdentifier), (it["title"] as? String) ?? "\u{01}", (it["description"] as? String) ?? "\u{01}"]
        args += stringList(it, "add").map { "+" + $0 }
        args += stringList(it, "remove").map { "-" + $0 }
        var ok = false
        for _ in 0..<3 {
            if (try? runAppleScript(keywordsWriteScript, args: args)) != nil { ok = true; break }
            Thread.sleep(forTimeInterval: 0.5)
        }
        if ok { changed += 1 } else { failed.append(["id": id, "error": "Photos refused the change three times"]) }
    }
    emit(["changed": changed, "failed": failed])
}

// MARK: - write side (PhotoKit)

func performChanges(_ block: @escaping () -> Void) throws {
    let sem = DispatchSemaphore(value: 0)
    var failure: Error?
    PHPhotoLibrary.shared().performChanges(block) { ok, err in
        if !ok { failure = err ?? HelperError(message: "change failed") }
        sem.signal()
    }
    _ = sem.wait(timeout: .now() + 3600)
    if let f = failure { throw f }
}

func cmdSet(_ p: [String: Any]) {
    requireAccess()
    let ids = stringList(p, "ids")
    if ids.isEmpty { fail("ids required") }
    let favorite = p["favorite"] as? Bool
    let hidden = p["hidden"] as? Bool
    if favorite == nil && hidden == nil { fail("favorite or hidden required") }
    let assets = fetchAssets(ids: ids)
    var changed = 0
    // Batches keep one bad asset from failing a whole library's worth of changes.
    for chunk in stride(from: 0, to: assets.count, by: 500) {
        let slice = Array(assets[chunk..<min(assets.count, chunk + 500)])
        do {
            try performChanges {
                for a in slice {
                    let req = PHAssetChangeRequest(for: a)
                    if let f = favorite { req.isFavorite = f }
                    if let h = hidden { req.isHidden = h }
                }
            }
            changed += slice.count
        } catch {
            for a in slice {
                if (try? performChanges {
                    let req = PHAssetChangeRequest(for: a)
                    if let f = favorite { req.isFavorite = f }
                    if let h = hidden { req.isHidden = h }
                }) != nil { changed += 1 }
            }
        }
    }
    emit(["changed": changed, "requested": assets.count])
}

func cmdFolderCreate(_ p: [String: Any]) {
    requireAccess()
    guard let title = p["title"] as? String, !title.isEmpty else { fail("title required") }
    var placeholder: PHObjectPlaceholder?
    do {
        try performChanges {
            let req = PHCollectionListChangeRequest.creationRequestForCollectionList(withTitle: title)
            placeholder = req.placeholderForCreatedCollectionList
            if let parentId = p["parent"] as? String, !parentId.isEmpty,
               let parent = PHCollectionList.fetchCollectionLists(withLocalIdentifiers: [parentId], options: nil).firstObject,
               let preq = PHCollectionListChangeRequest(for: parent) {
                preq.addChildCollections([req.placeholderForCreatedCollectionList] as NSFastEnumeration)
            }
        }
    } catch { fail(error.localizedDescription) }
    emit(["id": placeholder?.localIdentifier ?? ""])
}

func cmdAlbumCreate(_ p: [String: Any]) {
    requireAccess()
    guard let title = p["title"] as? String, !title.isEmpty else { fail("title required") }
    var placeholder: PHObjectPlaceholder?
    do {
        try performChanges {
            let req = PHAssetCollectionChangeRequest.creationRequestForAssetCollection(withTitle: title)
            placeholder = req.placeholderForCreatedAssetCollection
            if let folderId = p["folder"] as? String, !folderId.isEmpty,
               let folder = PHCollectionList.fetchCollectionLists(withLocalIdentifiers: [folderId], options: nil).firstObject,
               let freq = PHCollectionListChangeRequest(for: folder) {
                freq.addChildCollections([req.placeholderForCreatedAssetCollection] as NSFastEnumeration)
            }
        }
    } catch { fail(error.localizedDescription) }
    let id = placeholder?.localIdentifier ?? ""
    emit(["id": id, "cloudId": cloudIds(for: [id])[id] ?? ""])
}

func cmdAlbumAdd(_ p: [String: Any]) {
    requireAccess()
    guard let albumId = p["album"] as? String, !albumId.isEmpty else { fail("album required") }
    let ids = stringList(p, "ids")
    if ids.isEmpty { fail("ids required") }
    guard let album = PHAssetCollection.fetchAssetCollections(withLocalIdentifiers: [albumId], options: nil).firstObject else {
        fail("no album with id \(albumId)")
    }
    let assets = fetchAssets(ids: ids)
    do {
        try performChanges {
            PHAssetCollectionChangeRequest(for: album)?.addAssets(assets as NSFastEnumeration)
        }
    } catch { fail(error.localizedDescription) }
    // PhotoKit reports success even when it skipped something: count what is really there.
    let after = PHAsset.fetchAssets(in: album, options: nil)
    var present: Set<String> = []
    after.enumerateObjects { a, _, _ in present.insert(a.localIdentifier) }
    let added = assets.filter { present.contains($0.localIdentifier) }.count
    emit(["added": added, "requested": assets.count, "albumCount": after.count])
}


// MARK: - printing (milestone 2: the proof)

/// Runs a command-line tool and returns stdout (throws on a non-zero exit with stderr).
func runTool(_ path: String, _ args: [String], timeout: TimeInterval = 120) throws -> String {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: path)
    task.arguments = args
    let out = Pipe()
    let err = Pipe()
    task.standardOutput = out
    task.standardError = err
    try task.run()
    let data = out.fileHandleForReading.readDataToEndOfFile()
    let errData = err.fileHandleForReading.readDataToEndOfFile()
    task.waitUntilExit()
    if task.terminationStatus != 0 {
        let msg = String(data: errData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        throw HelperError(message: msg.isEmpty ? "\(path) exited with \(task.terminationStatus)" : msg)
    }
    return String(data: data, encoding: .utf8) ?? ""
}

/// What the queue's PPD says about its driver. The native Epson driver's PPD names an Epson
/// filter; the AirPrint queue macOS makes by itself is a generic URF/PCLm one.
func ppdInfo(queue: String) -> [String: Any] {
    let path = "/private/etc/cups/ppd/\(queue).ppd"
    guard let text = try? String(contentsOfFile: path, encoding: .isoLatin1) else {
        return ["ppd": false]
    }
    var info: [String: Any] = ["ppd": true, "ppdPath": path]
    var nick = ""
    var filters: [String] = []
    var manufacturer = ""
    var urf = false
    for line in text.split(separator: "\n") {
        let l = String(line)
        if l.hasPrefix("*NickName:") { nick = l.replacingOccurrences(of: "*NickName:", with: "").trimmingCharacters(in: CharacterSet(charactersIn: " \"")) }
        if l.hasPrefix("*Manufacturer:") { manufacturer = l.replacingOccurrences(of: "*Manufacturer:", with: "").trimmingCharacters(in: CharacterSet(charactersIn: " \"")) }
        if l.hasPrefix("*cupsFilter") { filters.append(l) }
        if l.contains("image/urf") || l.contains("application/PCLm") || l.hasPrefix("*cupsUrfSupported") { urf = true }
    }
    let epsonFilter = filters.contains { $0.lowercased().contains("epson") || $0.contains("/Library/Printers/EPSON") }
    info["nickName"] = nick
    info["manufacturer"] = manufacturer
    info["filters"] = filters
    info["airPrint"] = urf && !epsonFilter || nick.lowercased().contains("airprint")
    info["epsonDriver"] = epsonFilter
    return info
}

/// `print.queues` -> {items: [{name, model, isDefault, airPrint, epsonDriver, nickName, state}]}
func cmdPrintQueues() {
    var items: [[String: Any]] = []
    let defaultName = NSPrintInfo.defaultPrinter?.name ?? ""
    for name in NSPrinter.printerNames {
        var row: [String: Any] = ["name": name, "isDefault": name == defaultName]
        if let p = NSPrinter(name: name) {
            row["model"] = p.type
        }
        // CUPS queue name: NSPrinter names are display names; lpstat lists queue names
        row.merge(ppdInfo(queue: name)) { _, new in new }
        items.append(row)
    }
    // queues CUPS knows under another name than the display name
    if let lp = try? runTool("/usr/bin/lpstat", ["-p"]) {
        for line in lp.split(separator: "\n") where line.hasPrefix("printer ") {
            let parts = line.split(separator: " ")
            if parts.count >= 2 {
                let q = String(parts[1])
                if !items.contains(where: { ($0["name"] as? String) == q }) {
                    var row: [String: Any] = ["name": q, "isDefault": false, "state": String(line)]
                    row.merge(ppdInfo(queue: q)) { _, new in new }
                    items.append(row)
                }
            }
        }
    }
    emit(["items": items, "default": defaultName])
}

/// `print.options {queue}` -> {options: [{key, label, choices: [{value, label, current}]}]}
/// Everything the driver exposes, straight from `lpoptions -l`. This is how the exact names of
/// media type, quality, color mode and Advanced B&W are found on this Mac and this driver.
func cmdPrintOptions(_ p: [String: Any]) {
    guard let queue = p["queue"] as? String, !queue.isEmpty else { fail("queue required") }
    let text: String
    do { text = try runTool("/usr/bin/lpoptions", ["-p", queue, "-l"]) } catch { fail(error.localizedDescription) }
    var options: [[String: Any]] = []
    for line in text.split(separator: "\n") {
        // Key/Label: a b *c d
        guard let colon = line.firstIndex(of: ":") else { continue }
        let head = String(line[..<colon])
        let tail = String(line[line.index(after: colon)...]).trimmingCharacters(in: .whitespaces)
        let keyLabel = head.split(separator: "/", maxSplits: 1).map(String.init)
        let key = keyLabel.first ?? head
        let label = keyLabel.count > 1 ? keyLabel[1] : head
        var choices: [[String: Any]] = []
        for tok in tail.split(separator: " ") {
            var v = String(tok)
            let current = v.hasPrefix("*")
            if current { v.removeFirst() }
            choices.append(["value": v, "current": current])
        }
        options.append(["key": key, "label": label, "choices": choices])
    }
    // the human labels for each choice live in the PPD
    if let ppd = try? String(contentsOfFile: "/private/etc/cups/ppd/\(queue).ppd", encoding: .isoLatin1) {
        var labels: [String: String] = [:]
        for line in ppd.split(separator: "\n") where line.hasPrefix("*") && !line.hasPrefix("*%") {
            // *Key Value/Label: ...
            guard let colon = line.firstIndex(of: ":") else { continue }
            let head = String(line[line.index(after: line.startIndex)..<colon])
            let sp = head.split(separator: " ", maxSplits: 1).map(String.init)
            if sp.count == 2, let slash = sp[1].firstIndex(of: "/") {
                let value = String(sp[1][..<slash])
                let lab = String(sp[1][sp[1].index(after: slash)...])
                labels["\(sp[0])\u{1F}\(value)"] = lab
            }
        }
        options = options.map { opt in
            var o = opt
            let key = o["key"] as! String
            o["choices"] = (o["choices"] as! [[String: Any]]).map { c in
                var c2 = c
                if let l = labels["\(key)\u{1F}\(c["value"] as! String)"] { c2["label"] = l }
                return c2
            }
            return o
        }
    }
    emit(["queue": queue, "options": options])
}

/// `print.submit {queue, file, title?, copies?, options: {key: value}}` -> {jobId, request}
/// Hands a file the app has already prepared (16-bit TIFF in the paper's profile) to the queue
/// with the driver options named, through `lp`. The driver does the rest.
func cmdPrintSubmit(_ p: [String: Any]) {
    guard let queue = p["queue"] as? String, !queue.isEmpty else { fail("queue required") }
    guard let file = p["file"] as? String, FileManager.default.fileExists(atPath: file) else { fail("file required and must exist") }
    let info = ppdInfo(queue: queue)
    if info["airPrint"] as? Bool == true && p["allowAirPrint"] as? Bool != true {
        fail("\(queue) is an AirPrint queue. Darkroom prints through the native Epson driver only: add the printer again with Epson's driver (System Settings > Printers & Scanners, choose the EPSON SC-P900 Series driver, not AirPrint).")
    }
    var args = ["-d", queue]
    if let title = p["title"] as? String, !title.isEmpty { args += ["-t", title] }
    if let copies = p["copies"] as? Int, copies > 1 { args += ["-n", String(copies)] }
    var used: [String] = []
    if let opts = p["options"] as? [String: Any] {
        for (k, v) in opts.sorted(by: { $0.key < $1.key }) {
            let val: String
            if let b = v as? Bool { val = b ? "True" : "False" } else { val = "\(v)" }
            args += ["-o", "\(k)=\(val)"]
            used.append("\(k)=\(val)")
        }
    }
    args.append(file)
    let out: String
    do { out = try runTool("/usr/bin/lp", args) } catch { fail(error.localizedDescription) }
    // "request id is P900-42 (1 file(s))"
    var jobId = ""
    if let r = out.range(of: "request id is ") {
        jobId = String(out[r.upperBound...]).split(separator: " ").first.map(String.init) ?? ""
    }
    emit(["jobId": jobId, "request": out.trimmingCharacters(in: .whitespacesAndNewlines), "options": used, "queue": queue])
}

/// `print.jobs {queue?}` -> {items: [..]} what is in the queue (lpstat -o).
func cmdPrintJobs(_ p: [String: Any]) {
    var args = ["-o"]
    if let q = p["queue"] as? String, !q.isEmpty { args.append(q) }
    let out = (try? runTool("/usr/bin/lpstat", args)) ?? ""
    emit(["items": out.split(separator: "\n").map(String.init)])
}

// MARK: - main

let (command, params) = readArgs()
switch command {
case "status": cmdStatus()
case "albums": cmdAlbums()
case "assets": cmdAssets(params)
case "originals": cmdOriginals(params)
case "thumbs": cmdThumbs(params)
case "keywords": cmdKeywords(params)
case "keywords.set": cmdKeywordsSet(params)
case "set": cmdSet(params)
case "album.create": cmdAlbumCreate(params)
case "folder.create": cmdFolderCreate(params)
case "album.add": cmdAlbumAdd(params)
case "print.queues": cmdPrintQueues()
case "print.options": cmdPrintOptions(params)
case "print.submit": cmdPrintSubmit(params)
case "print.jobs": cmdPrintJobs(params)
case "version": emit(["version": "0.1.0", "commands": ["status", "albums", "assets", "originals", "thumbs", "keywords", "keywords.set", "set", "album.create", "folder.create", "album.add", "print.queues", "print.options", "print.submit", "print.jobs"]])
default: fail("unknown command \(command)")
}

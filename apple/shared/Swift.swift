/// A wrapper for objects that gives them identity based on their address.
class ObjectId<V>: Equatable, Hashable {
    let value: V

    init(_ v: V) {
        self.value = v
    }

    static func == (l: ObjectId<V>, r: ObjectId<V>) -> Bool {
        return l === r
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(ObjectIdentifier(self))
    }
}

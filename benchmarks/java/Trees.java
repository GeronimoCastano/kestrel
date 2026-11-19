// The Benchmarks Game binary-trees program, single-threaded.
public class Trees {
    static final class Node {
        final Node left, right;
        Node(Node left, Node right) { this.left = left; this.right = right; }
    }
    static Node make(int depth) {
        return depth == 0 ? new Node(null, null) : new Node(make(depth - 1), make(depth - 1));
    }
    static long check(Node t) {
        return t == null ? 0 : 1 + check(t.left) + check(t.right);
    }
    public static void main(String[] args) {
        int maxDepth = 18, stretch = maxDepth + 1;
        System.out.println("stretch tree of depth " + stretch + "\t check: " + check(make(stretch)));
        Node longLived = make(maxDepth);
        for (int depth = 4; depth <= maxDepth; depth += 2) {
            long iterations = 1L << (maxDepth - depth + 4), total = 0;
            for (long i = 0; i < iterations; i++) total += check(make(depth));
            System.out.println(iterations + "\t trees of depth " + depth + "\t check: " + total);
        }
        System.out.println("long lived tree of depth " + maxDepth + "\t check: " + check(longLived));
    }
}

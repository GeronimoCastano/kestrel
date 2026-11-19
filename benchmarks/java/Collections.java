import java.util.ArrayList;
import java.util.HashMap;
import java.util.Map;

public class Collections {
    static long build(long n) {
        HashMap<Long, Long> m = new HashMap<>();
        for (long i = 0; i < n; i++) m.put(i, i * 2);
        long total = 0;
        for (long v : m.values()) total += v;
        ArrayList<Long> items = new ArrayList<>();
        for (Map.Entry<Long, Long> e : m.entrySet()) items.add(e.getValue() + e.getKey());
        long listTotal = 0;
        for (long x : items) listTotal += x;
        return total + listTotal;
    }
    public static void main(String[] args) {
        System.out.println(build(2_000_000L));
    }
}

public class Numeric {
    static long lcg(long n) {
        long x = 42, total = 0;
        for (long i = 0; i < n; i++) {
            x = (x * 1103515245L + 12345) % 2147483648L;
            total += x % 1000;
        }
        return total;
    }
    static double leibniz(long n) {
        double sum = 0.0, sign = 1.0;
        for (long i = 0; i < n; i++) {
            sum += sign / (2.0 * i + 1.0);
            sign = -sign;
        }
        return 4.0 * sum;
    }
    public static void main(String[] args) {
        System.out.println(lcg(200_000_000L));
        System.out.println(String.format("%.9f", leibniz(200_000_000L)));
    }
}

public class Strings {
    static long build(long n) {
        StringBuilder s = new StringBuilder();
        for (long i = 0; i < n; i++) s.append(i).append(',');
        return s.toString().codePointCount(0, s.length());
    }
    public static void main(String[] args) {
        System.out.println(build(2_000_000L));
    }
}

set terminal svg enhanced background rgb "#0D1117" size 1280 720

set linetype 1 lw 2 lc rgb '#73D0FF' pointtype 6
set linetype 2 lw 2 lc rgb '#FFD173' pointtype 6
set linetype 3 lw 2 lc rgb '#D5FF80' pointtype 6
set linetype 4 lw 2 lc rgb '#F27983' pointtype 6
set linetype 5 lw 2 lc rgb '#DFBFFF' pointtype 6
set linetype 6 lw 2 lc rgb '#BFBDB6' pointtype 6
set linetype 7 lw 2 lc rgb '#FF6666' pointtype 6
set title textcolor rgb '#E6EDF3' font "Verdana,18"
set ylabel textcolor rgb '#E6EDF3' font "Verdana,18"
set xlabel textcolor rgb '#E6EDF3' font "Verdana,18"
set style fill solid border rgb "#1A1F29"
set border lw 3 lc rgb '#E6EDF3'
set key textcolor rgb '#E6EDF3' font "Verdana,14"
set xtics nomirror
set border lw 1

set output "{id}.svg"
set title "users"
# set logscale y
set ytics 50
set mytics 2
set style data histogram
set style histogram cluster gap 1
set ylabel "change in user count in period"
set xlabel "days"
set auto x
set yrange [{floor}:*]
set style line 12 lc rgb '#1F2430' lt 1 lw 2 dt 22
unset xtics
set xtics format ""
set xtics scale 0
set xtics rotate by 45 right
set grid ytics ls 12
set datafile separator ","
set key top left

plot '{id}.dat' u 2:xtic(1) smooth acsplines title "user count"
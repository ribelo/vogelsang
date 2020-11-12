(ns vogelsang.allocation
  (:require
   [taoensso.encore :as e]
   [datalevin.core :as d]
   [hanse.danzig :refer [comp-some =>> +>>]]
   [hanse.halle :as h]
   [hanse.lubeck.quant :as quant]
   [hanse.rostock.math :as math]
   [hanse.rostock.emath :as emath]
   [hanse.rostock.stats :as stats]
   [hanse.lubeck.redp :as redp]
   [vogelsang.db :as db]
   [vogelsang.data :as data]
   [java-time :as jt]
   [net.cgrand.xforms :as x]
   [cuerdas.core :as str]))

(defn money-allocation [^double money]
  (comp
    (keep (fn [{:keys [^double allocation ^double last-price] :as m}]
            (assoc m
                   :allocation (math/round allocation 2)
                   :money (math/round (* money allocation) 2))))
    (x/sort-by :allocation e/rcompare)))

(def single-allocation
  (e/memoize
      (fn
        ([symbol]
         (single-allocation symbol {}))
        ([symbol {:keys [n frisk risk money drop-last-month?]
                  :or   {frisk            0.0
                         risk             0.3
                         drop-last-month? false}}]
         (when-let [data (->> (data/quotes symbol :month :close 13) (drop-last (if drop-last-month? 1 0)))]
           (redp/single-allocation frisk risk 12 data))))))

(defn add-single-allocation
  ([symbol]
   (add-single-allocation symbol {}))
  ([symbol {:keys [n frisk risk money]
            :or   {frisk 0.0
                   risk  0.3}
            :as   opts}]
   (e/when-lets [symbol       (-> symbol str/lower str/kebab)
                 vogelsang-id (e/merge-keywords [:yf symbol :quant])
                 redp         (single-allocation symbol opts)
                 doc          {:yf.quant/single-allocation redp :vogelsang/id vogelsang-id}]
     (d/transact! db/conn [doc]))))

(def multiple-allocations
  (e/memoize
      (fn
        ([symbols] (multiple-allocations symbols {}))
        ([symbols {:keys [n frisk risk money drop-last-month?]
                   :or   {n                24
                          frisk            0.0
                          risk             0.3
                          money            100000.00
                          drop-last-month? false}}]
         (let [assets (+>> symbols
                           (keep (fn [symbol]
                                   (when-let [data (not-empty (->> (data/quotes symbol :month :close 13)
                                                                   (drop-last (if drop-last-month? 1 0))))]
                                     {symbol data}))))]
           (loop [i 0 assets assets]
             (if (< i 1000)
               (let [allocations       (=>> (redp/multiple-allocation frisk risk 12 (vec (vals assets)))
                                            (map-indexed (fn [i p]
                                                           (let [symbol (nth (keys assets) i)]
                                                             {:symbol     symbol
                                                              :allocation p
                                                              :last-price (data/last-close-price symbol)})))
                                            (x/sort-by :allocation e/rcompare)
                                            (take (+ ^long n 5)))
                     money-allocations (=>> allocations
                                            (money-allocation money))
                     symbols           (into #{} (comp (x/drop-last) (map :symbol)) allocations)
                     new-assets        (=>> assets
                                            (filter (fn [[symbol _]]
                                                      (contains? symbols symbol))))]
                 (if (> (count assets) ^long n)
                   (recur (inc i) new-assets)
                   money-allocations)))))))))

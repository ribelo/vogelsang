(ns vogelsang.api
  (:require
   [taoensso.encore :as e]
   [clojure.set]
   [datalevin.core :as d]
   [taoensso.timbre :as timbre]
   [vogelsang.db :as db]
   [vogelsang.data :as data]
   [vogelsang.allocation]
   [vogelsang.quant]
   [hanse.danzig :refer [=>> +>>]]
   [ribelo.torgau :as org]
   [cuerdas.core :as str]
   [hanse.rostock.stats :as stats]
   [hanse.rostock.math :as math]
   [hanse.lubeck.quant :as quant]
   [hanse.lubeck.redp :as redp]
   [net.cgrand.xforms :as x]
   [uncomplicate.fluokitten.jvm]
   [uncomplicate.fluokitten.core :as fk]
   [meander.epsilon :as r]))

(defn refresh-quant! [symbol]
  (e/catching (vogelsang.quant/refresh-analyse symbol) e
    (timbre/error symbol (ex-message e)))
  (e/catching (vogelsang.allocation/add-single-allocation symbol) e
    (timbre/error symbol (ex-message e))))

(defn refresh-quants! []
  (let [n (count (vogelsang.data/symbols-to-download))
        i (atom 0)]
    (doseq [symbol (shuffle (vogelsang.data/symbols-to-download))]
      (refresh-quant! symbol)
      (swap! i inc)
      (timbre/infof "refresh %s" symbol)
      (timbre/infof "%.2f%%\n" (double (* 100 (/ @i n)))))))

(defn refresh-symbol! [symbol]
  (e/catching (vogelsang.data/refresh-yf-data symbol) e
    (timbre/error symbol (ex-message e))))

(defn delete-symbol! [symbol]
  (e/catching (do (vogelsang.data/delete-quotes symbol)
                  (vogelsang.data/evict-documents-by-symbol symbol)) e
    (timbre/error symbol (ex-message e))))

(defn refresh-symbols! []
  (e/when-lets [n (some-> (vogelsang.data/symbols-to-download) (count))
                i (atom 0)]
    (doseq [symbol (shuffle (vogelsang.data/symbols-to-download))]
      (refresh-symbol! symbol)
      (swap! i inc)
      (timbre/infof "%.2f%%" (double (* 100 (/ @i n)))))))

(defn check-downloaded-symbols! []
  (let [download_all_ (atom false)]
    (doseq [symbol (vogelsang.data/symbols-to-download)]
      (loop []
        (let [n (count (vogelsang.data/quotes :mem/fresh symbol))]
          (when (< n 30)
            (printf "%s - %d : not enough data\n" symbol n)
            (println "try to download again? [y/n/all]")
            (let [r (read-line)]
              (when (= "all" r) (reset! download_all_ true))
              (when (or @download_all_ (#{"y" "Y"} r ))
                (e/catching
                    (do (vogelsang.data/delete-quotes symbol)
                        (vogelsang.data/refresh-yf-data symbol))
                    e (timbre/error e))
                (recur)))))))))

(defn portfolio [{:keys [exclude min-sharpe max-dd max-redp max-price max-count money drop-last-month]
                  :or   {min-sharpe      1.0
                         max-dd          0.3
                         max-redp        1.0
                         max-price       100000.0
                         max-count       5
                         money           100000
                         drop-last-month false}
                  :as   opts}]
  (let [symbols (=>> (d/q '[:find  [?symbol ...]
                            :in $ ?round-fn ?min-sharpe ?max-dd ?max-redp ?max-price
                            :where
                            [?e :yf/symbol ?symbol]
                            [?e :yf.quant/sharpe-ratio ?sharpe]
                            [(>= ?sharpe ?min-sharpe)]
                            [?e :yf.quant/maximum-drawdown ?dd]
                            [(<= ?dd ?max-dd)]
                            [?e :yf.quant/single-allocation ?allocation]
                            [(= 1.0 ?allocation)]
                            [?e :yf.quant/redp ?redp]
                            [(<= ?redp ?max-redp)]
                            [(vogelsang.data/last-close-price ?symbol) ?last-price]
                            [(?round-fn ?last-price) ?last-price]
                            [(<= ?last-price ?max-price)]]
                       @db/conn math/round2 min-sharpe max-dd max-redp max-price))
        symbols (clojure.set/difference (into #{} symbols) exclude)]
    (org/table
      [:symbol :name :allocation :money :avg-dd :max-dd :price :stop :redp :ann-ret :sharpe :beta]
      (=>> (vogelsang.allocation/multiple-allocations symbols
                                                      {:n                max-count
                                                       :money            money
                                                       :drop-last-month? drop-last-month})
           (map (fn [{:keys [symbol] :as m}]
                  (-> m
                      (dissoc :qty)
                      (clojure.set/rename-keys {:last-price :price})
                      (update :price math/round2)
                      (assoc :name    (data/document symbol :yf.info/name)
                             :avg-dd  (some->> (data/document symbol :yf.quant/average-drawdown) e/as-?float (format "%.2f"))
                             :max-dd  (some->> (data/document symbol :yf.quant/maximum-drawdown) e/as-?float (format "%.2f"))
                             :stop    (some->> (* (data/last-close-price symbol)
                                                  (- 1 (data/document symbol :yf.quant/average-drawdown)))
                                               e/as-?float (format "%.2f"))
                             :redp    (some->> (data/document symbol :yf.quant/redp) e/as-?float (format "%.2f"))
                             :ann-ret (some->> (data/document symbol :yf.quant/annualized-return) e/as-?float  (format "%.2f"))
                             :sharpe  (some->> (data/document symbol :yf.quant/sharpe-ratio) e/as-?float (format "%.2f"))
                             :calmar  (some->> (data/document symbol :yf.quant/calmar-ratio) e/as-?float (format "%.2f"))
                             :beta    (some->> (data/document symbol :yf.price-history/beta) e/as-?float (format "%.2f"))))))))
    ))

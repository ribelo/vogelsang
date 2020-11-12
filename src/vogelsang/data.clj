(ns vogelsang.data
  (:require
   [clojure.core.async :as a]
   [ribelo.yf :as yf]
   [vogelsang.db :as db]
   [java-time :as jt]
   [cuerdas.core :as str]
   [taoensso.timbre :as timbre]
   [datalevin.core :as d]
   [taoensso.encore :as e]
   [net.cgrand.xforms :as x]
   [hanse.danzig :refer [=>>]]
   [meander.epsilon :as m]
   [hanse.lubeck.quant :as quant]))

(defn symbols-to-download []
  (:vogelsang.data/symbols (d/entity @db/conn 1)))

(defn add-symbol-to-download [symbol]
  (d/transact! db/conn [[:db/add 1 :vogelsang.data/symbols symbol]])
  (timbre/info :add-symbol-to-download symbol))

(defn set-symbols-to-download [symbols]
  (d/transact! db/conn [[:db.fn/retractAttribute 1 :vogelsang.data/symbols]])
  (doseq [symbol symbols] (add-symbol-to-download symbol))
  (timbre/info :set-symbols-to-download))

(defn remove-symbol-to-download [symbol]
  (let [symbol (-> symbol (name) (str/lower) (str/kebab))]
    (d/transact! db/conn [[:db/retract 1 :vogelsang.data/symbols symbol]])
    (timbre/info :remove-symbol-to-download symbol)))

(defn remove-all-symbols-to-download []
  (d/transact! db/conn [[:db.fn/retractAttribute 1 :vogelsang.data/symbols]])
  (timbre/info :remove-all-symbols-to-download))

(defn yf-data->doc [symbol name doc]
  (let [symbol (-> symbol (str/lower) (str/kebab))]
    (reduce-kv
      (fn [acc k v]
        (if v (assoc acc (e/merge-keywords [:yf name k]) v) acc))
      {:yf/symbol    symbol
       :vogelsang/id (e/merge-keywords [:yf symbol name])}
      doc)))

(defn merge-document [eid doc]
  (let [merge-fn (fn [db eid doc]
                   (println (merge (d/pull db '[*] eid) doc))
                   (let [doc* (merge (d/pull db '[*] eid) doc)] [doc*]))]
    (d/transact! db/conn [[:db.fn/call merge-fn eid doc]])))

(defmacro register-setter [k fn]
  (let [add-name (str "add-" (name k))]
    `(do
       (defn ~(symbol add-name) [symbol#]
         (let [symbol*# (-> symbol# name str/lower str/kebab)
               id#      (e/merge-keywords [:yf symbol*# ~k])]
           (timbre/debug (e/merge-keywords [:yf symbol*# ~k]))
           (when-let [data# (~fn (-> symbol# name str/lower))]
             (->> data#
                  (yf-data->doc symbol*# ~k)
                  (conj [])
                  (d/transact! db/conn))))))))

(defn evict-documents-by-symbol [symbol]
  (let [symbol (-> symbol name str/lower str/kebab)
        eids   (d/q '[:find [?e ...] :in $ ?symbol :where [?e :yf/symbol ?symbol]]
                 @db/conn symbol)]
    (d/transact! db/conn (=>> eids (map (fn [eid] [:db/retractEntity eid]))))))

(do (register-setter :info yf/company-info)
    (register-setter :valuation yf/valuation)
    (register-setter :fiscal-year yf/fiscal-year)
    (register-setter :profitability yf/profitability)
    (register-setter :effectivness yf/management-effectivness)
    (register-setter :income yf/income-statement)
    (register-setter :balance yf/balance-sheet)
    (register-setter :cash-flow yf/cash-flow-statement)
    (register-setter :price-history yf/stock-price-history)
    (register-setter :shares yf/share-statistics)
    (register-setter :dividends yf/dividends))

(defn document
  ([symbol]
   (document symbol nil))
  ([symbol info]
   (let [symbol (-> symbol (name) (str/lower) (str/kebab))]
     (m/match info
       (m/or :all nil)
       (->> (d/q '[:find [(pull ?e [*]) ...] :in $ ?symbol :where [?e :yf/symbol ?symbol]]
              @db/conn symbol)
            (apply merge)
            (into (sorted-map)))
       ;;
       (m/pred e/qualified-keyword?)
       (-> (d/q '[:find ?v .
                  :in $ ?symbol ?info
                  :where
                  [?e :yf/symbol ?symbol]
                  [?e ?info ?v]]
             @db/conn symbol info))
       ;;
       (m/pred e/simple-keyword?)
       (->> (document symbol :all)
            (reduce-kv
              (fn [acc k v]
                (if (re-find (re-pattern (e/qname info)) (e/qname k))
                  (assoc acc k v)
                  acc))
              (sorted-map)))
       ;;
       (m/pred coll?)
       (-> (document symbol :all)
           (select-keys info))))))

(defn resample-data [period]
  (m/match period
    :day   (map identity)
    :month (comp
             (x/by-key (fn [[date & _]] (jt/as (jt/local-date date) :year :month-of-year))
                       (x/take-last 1))
             (map second)
             (x/sort-by first))
    :year  (comp
             (x/by-key (fn [[date & _]] (jt/as (jt/local-date date) :year))
                       (x/take-last 1))
             (map second)
             (x/sort-by first))))

(defn last-quotes-date [symbol]
  (let [symbol (-> symbol name str/lower str/kebab)]
    (=>> (d/q '[:find     [?dt ...]
                :in $ ?symbol
                :where
                [?e :yf.quotes/symbol ?symbol]
                [?e :yf.quotes/date ?dt]]
           @db/conn symbol)
         (x/sort e/rcompare)
         .)))

(defn- quotes->docs [symbol quotes]
  (let [symbol (-> symbol str/lower str/kebab)]
    (=>> quotes
         (e/xdistinct :date)
         (map (fn [doc]
                (as-> (dissoc (yf-data->doc symbol :quotes doc) :yf/symbol) doc*
                  (assoc doc* :yf.quotes/symbol symbol
                         :vogelsang/id (e/merge-keywords [:yf symbol :quotes (:yf.quotes/date doc*)]))))))))

(defn add-quotes
  ([symbol]
   (let [last-date (or (last-quotes-date symbol) (jt/minus (jt/local-date-time) (jt/months 13)))]
     (add-quotes symbol last-date)))
  ([symbol last-date]
   (let [symbol (-> symbol name str/lower)
         docs   (->> (yf/get-data symbol {:start last-date})
                     (remove (fn [{:keys [date]}] (= last-date date)))
                     (quotes->docs symbol))]
     (timbre/debug (e/merge-keywords [:yf (str/kebab symbol) :quotes]) :from last-date)
     (d/transact! db/conn docs))))

(def quotes
  (e/memoize (e/ms :mins 5)
    (fn
      ([symbol]
       (let [symbol (-> symbol name str/lower str/kebab)]
         (=>> (d/q '[:find ?dt ?o ?h ?l ?c ?v
                     :in $ ?symbol
                     :where
                     [?e :yf.quotes/symbol ?symbol]
                     [?e :yf.quotes/date ?dt]
                     [?e :yf.quotes/open ?o]
                     [?e :yf.quotes/high ?h]
                     [?e :yf.quotes/low ?l]
                     [?e :yf.quotes/close ?c]
                     [?e :yf.quotes/volume ?v]]
                @db/conn symbol)
              (x/sort-by first))))
      ([symbol period]
       (=>> (quotes symbol)
            (resample-data period)))
      ([symbol period k]
       (=>> (quotes symbol period)
            (map (fn [[dt o h l c v]]
                   (case k
                     :date   dt
                     :open   o
                     :high   h
                     :low    l
                     :close  c
                     :volume v)))))
      ([symbol period k n]
       (=>> (quotes symbol period k)
            (x/take-last n))))))

(defn return
  ([symbol]
   (return symbol :day))
  ([symbol period]
   (->> (quotes symbol period :close)
        (quant/tick->ret)))
  ([symbol period n]
   (->> (quotes symbol period :close)
        (take-last n)
        (quant/tick->ret))))

(defn delete-quotes [symbol]
  (let [symbol (-> symbol (name) (str/lower) (str/kebab))]
    (->> (d/q '[:find  [?e ...]
                :in $ ?symbol
                :where
                (or [?e :yf.quotes/symbol ?symbol]
                    [?e :yf/symbol ?symbol])]
           @db/conn symbol)
         (mapv (fn [?e] [:db/retractEntity ?e]))
         (d/transact! db/conn))))

(defn delete-all-data []
  (->> (d/q '[:find  [?e ...]
              :where [?e]]
         @db/conn)
       (mapv (fn [?e] [:db/retractEntity ?e]))
       (d/transact! db/conn)))

(defn refresh-yf-data [symbol]
  (add-quotes symbol)
  (add-info symbol)
  (add-valuation symbol)
  (add-profitability symbol)
  (add-income symbol)
  (add-cash-flow symbol)
  (add-shares symbol)
  (add-price-history symbol)
  (add-balance symbol)
  (add-effectivness symbol)
  (add-fiscal-year symbol)
  (add-dividends symbol))

(def last-close-price
  (e/memoize
      (fn [symbol]
        (let [symbol (-> symbol (name) (str/lower) (str/kebab))]
          (->> (d/q '[:find     ?dt ?c
                      :in $ ?symbol
                      :where
                      [?e :yf.quotes/symbol ?symbol]
                      [?e :yf.quotes/date ?dt]
                      [?e :yf.quotes/close ?c]]
                 @db/conn symbol)
               (sort-by first e/rcompare)
               (first)
               (last))))))
